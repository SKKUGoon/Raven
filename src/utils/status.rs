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
use std::collections::BTreeMap;

pub async fn check_status(settings: &Settings) {
    let services = service_registry::all_services(settings);
    let host = service_registry::client_host(&settings.server.host);

    let service_width = services
        .iter()
        .map(|svc| svc.display_name.len())
        .max()
        .unwrap_or(0)
        .max("Service".len());
    println!(
        "{:<service_width$} | {:<10} | {:<10}",
        "Service",
        "Port",
        "Status",
        service_width = service_width
    );
    println!(
        "{:-<service_width$}-|-{:-<10}-|-{:-<10}",
        "",
        "",
        "",
        service_width = service_width
    );

    let mut kline_port: Option<u16> = None;
    let mut kline_collections: Option<usize> = None;
    let mut kline_list_ok: Option<bool> = None;
    let mut kline_metrics_parsed: Option<(BTreeMap<usize, i64>, BTreeMap<usize, i64>)> = None;

    for svc in services {
        let addr = svc.addr(host);
        let mut list_ok = false;
        let mut list_count = None;
        let mut healthy = match ControlClient::connect(addr).await {
            Ok(mut client) => match client.list_collections(ListRequest {}).await {
                Ok(resp) => {
                    list_ok = true;
                    list_count = Some(resp.into_inner().collections.len());
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        };

        if svc.id == "binance_futures_klines" {
            let metrics_port = svc.port.saturating_add(1000);
            let metrics = fetch_metrics(host, metrics_port).await;
            if let Some(metrics) = metrics {
                let parsed = parse_kline_shard_metrics(&metrics);
                let shard_connected = parsed.0.values().any(|v| *v > 0);
                if shard_connected {
                    healthy = true;
                }
                kline_metrics_parsed = Some(parsed);
            }
        }
        let status = if healthy {
            "\x1b[32mHEALTHY\x1b[0m" // Green
        } else {
            "\x1b[31mUNHEALTHY\x1b[0m" // Red
        };
        println!(
            "{:<service_width$} | {:<10} | {}",
            svc.display_name,
            svc.port,
            status,
            service_width = service_width
        );

        if svc.id == "binance_futures_klines" {
            kline_port = Some(svc.port);
            kline_collections = list_count;
            kline_list_ok = Some(list_ok);
        }
    }

    if let Some(port) = kline_port {
        println!();
        println!("BINANCE FUTURES KLINES");

        let metrics_port = port.saturating_add(1000);
        let shard_count = settings.binance_klines.connections.max(1);
        let shard_size = settings.binance_klines.shard_size.max(1);
        let capacity = shard_count.saturating_mul(shard_size);
        if let Some(count) = kline_collections {
            if count >= capacity {
                println!(
                    "Active collections: {count} (at capacity: connections={shard_count} * shard_size={shard_size})"
                );
            } else {
                println!(
                    "Active collections: {count} (capacity: connections={shard_count} * shard_size={shard_size})"
                );
            }
        }
        if kline_list_ok == Some(false) {
            println!("Control API check failed; attempting metrics scrape anyway.");
        }
        match kline_metrics_parsed {
            Some((connected, streams)) => {
                if connected.is_empty() && streams.is_empty() {
                    println!("No shard metrics found (scraped {host}:{metrics_port})");
                } else {
                    for shard_idx in 0..shard_count {
                        let is_up = connected.get(&shard_idx).copied().unwrap_or(0) > 0;
                        let status = if is_up {
                            "\x1b[32mHEALTHY\x1b[0m"
                        } else {
                            "\x1b[31mUNHEALTHY\x1b[0m"
                        };
                        let display_idx = shard_idx + 1;
                        let display_label = format!("{display_idx:02}");
                        println!("    {display_label}. SHARD {display_label:<26}: {status}");
                    }
                }
            }
            None => {
                println!("Failed to scrape shard metrics from {host}:{metrics_port}");
            }
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
