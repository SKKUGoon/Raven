use crate::config::Settings;
use crate::proto::control_client::ControlClient;
use crate::proto::ListRequest;
use crate::utils::service_registry;
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper::Uri;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::{BTreeMap, BTreeSet};

pub async fn check_status(settings: &Settings) {
    let services = service_registry::all_services(settings);
    let host = service_registry::client_host(&settings.server.host);

    println!("{:<20} | {:<10} | {:<10}", "Service", "Port", "Status");
    println!("{:-<20}-|-{:-<10}-|-{:-<10}", "", "", "");

    let mut kline_port: Option<u16> = None;
    let mut kline_healthy: Option<bool> = None;

    for svc in services {
        let addr = svc.addr(host);
        let healthy = match ControlClient::connect(addr).await {
            Ok(mut client) => client.list_collections(ListRequest {}).await.is_ok(),
            Err(_) => false,
        };
        let status = if healthy {
            "\x1b[32mHEALTHY\x1b[0m" // Green
        } else {
            "\x1b[31mUNHEALTHY\x1b[0m" // Red
        };
        println!("{:<20} | {:<10} | {}", svc.display_name, svc.port, status);

        if svc.id == "binance_futures_klines" {
            kline_port = Some(svc.port);
            kline_healthy = Some(healthy);
        }
    }

    if let Some(port) = kline_port {
        println!();
        println!("Binance Futures Kline Shards");
        println!("{:-<36}", "");

        if kline_healthy == Some(true) {
            let metrics_port = port.saturating_add(1000);
            match fetch_metrics(host, metrics_port).await {
                Some(metrics) => {
                    let (connected, streams) = parse_kline_shard_metrics(&metrics);
                    if connected.is_empty() && streams.is_empty() {
                        println!("No shard metrics found (scraped {host}:{metrics_port})");
                    } else {
                        let mut shard_ids: BTreeSet<usize> = connected.keys().copied().collect();
                        shard_ids.extend(streams.keys().copied());

                        println!("{:<8} | {:<10} | {:<10}", "Shard", "Status", "Streams");
                        println!("{:-<8}-|-{:-<10}-|-{:-<10}", "", "", "");
                        for shard_idx in shard_ids {
                            let is_up = connected.get(&shard_idx).copied().unwrap_or(0) > 0;
                            let status = if is_up {
                                "\x1b[32mHEALTHY\x1b[0m"
                            } else {
                                "\x1b[31mUNHEALTHY\x1b[0m"
                            };
                            let stream_count = streams.get(&shard_idx).copied().unwrap_or(0);
                            println!("{shard_idx:<8} | {status:<10} | {stream_count:<10}");
                        }
                    }
                }
                None => {
                    println!("Failed to scrape shard metrics from {host}:{metrics_port}");
                }
            }
        } else {
            println!("Service unhealthy; skipping shard health.");
        }
    }
}

async fn fetch_metrics(host: &str, port: u16) -> Option<String> {
    let uri: Uri = format!("http://{host}:{port}/metrics").parse().ok()?;
    let client = Client::builder(TokioExecutor::new()).build_http();
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Empty::<Bytes>::new())
        .ok()?;
    let resp = client.request(req).await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.into_body().collect().await.ok()?.to_bytes();
    Some(String::from_utf8_lossy(bytes.as_ref()).to_string())
}

fn parse_kline_shard_metrics(metrics: &str) -> (BTreeMap<usize, i64>, BTreeMap<usize, i64>) {
    const CONNECTED_PREFIX: &str = "raven_binance_futures_klines_shard_connected";
    const STREAMS_PREFIX: &str = "raven_binance_futures_klines_shard_streams";
    let mut connected = BTreeMap::new();
    let mut streams = BTreeMap::new();

    for line in metrics.lines().map(str::trim) {
        if let Some((shard_idx, value)) = parse_metric_line(line, CONNECTED_PREFIX) {
            connected.insert(shard_idx, value);
        } else if let Some((shard_idx, value)) = parse_metric_line(line, STREAMS_PREFIX) {
            streams.insert(shard_idx, value);
        }
    }

    (connected, streams)
}

fn parse_metric_line(line: &str, prefix: &str) -> Option<(usize, i64)> {
    let rest = line.strip_prefix(prefix)?;
    let (labels, value_str) = rest.trim().split_once(' ')?;
    let shard_idx = parse_shard_label(labels)?;
    let value = parse_metric_value(value_str)?;
    Some((shard_idx, value))
}

fn parse_shard_label(labels: &str) -> Option<usize> {
    let labels = labels.strip_prefix('{')?.strip_suffix('}')?;
    let key = "shard=\"";
    let start = labels.find(key)? + key.len();
    let end = labels[start..].find('"')? + start;
    labels[start..end].parse().ok()
}

fn parse_metric_value(value_str: &str) -> Option<i64> {
    let value = value_str.split_whitespace().next()?;
    let value: f64 = value.parse().ok()?;
    Some(value.round() as i64)
}
