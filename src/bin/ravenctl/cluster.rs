use raven::config::Settings;
use raven::proto::control_client::ControlClient;
use raven::proto::{ListRequest, StopAllRequest};
use raven::utils::service_registry;

pub async fn stop_all_collections_cluster(settings: &Settings) {
    let host_ip = service_registry::client_host(&settings.server.host);
    let services = service_registry::all_services(settings);

    println!(
        "Stopping all collections across {} services...",
        services.len()
    );
    for svc in services {
        let addr = svc.addr(host_ip);
        match ControlClient::connect(addr.clone()).await {
            Ok(mut client) => match client.stop_all_collections(StopAllRequest {}).await {
                Ok(resp) => {
                    let inner = resp.into_inner();
                    println!(
                        "- {} ({}) @ {} -> success={} msg={}",
                        svc.display_name, svc.id, addr, inner.success, inner.message
                    );
                }
                Err(e) => println!(
                    "- {} ({}) @ {} -> ERROR: {e}",
                    svc.display_name, svc.id, addr
                ),
            },
            Err(e) => println!(
                "- {} ({}) @ {} -> UNREACHABLE: {e}",
                svc.display_name, svc.id, addr
            ),
        }
    }
}

pub async fn list_collections_cluster(settings: &Settings) {
    let host_ip = service_registry::client_host(&settings.server.host);
    let services = service_registry::all_services(settings);
    let excluded = ["binance_futures_klines", "kline_persistence"];

    println!("Active collections across services (excluding klines):");
    for svc in services {
        if excluded.contains(&svc.id) {
            continue;
        }
        let addr = svc.addr(host_ip);
        match ControlClient::connect(addr.clone()).await {
            Ok(mut client) => match client.list_collections(ListRequest {}).await {
                Ok(resp) => {
                    let collections = resp.into_inner().collections;
                    println!("- {} ({}) @ {}", svc.display_name, svc.id, addr);
                    if collections.is_empty() {
                        println!("  (none)");
                    } else {
                        for collection in collections {
                            println!(
                                "  Symbol: {}, Status: {}, Subscribers: {}",
                                collection.symbol, collection.status, collection.subscriber_count
                            );
                        }
                    }
                }
                Err(e) => println!(
                    "- {} ({}) @ {} -> ERROR: {e}",
                    svc.display_name, svc.id, addr
                ),
            },
            Err(e) => println!(
                "- {} ({}) @ {} -> UNREACHABLE: {e}",
                svc.display_name, svc.id, addr
            ),
        }
    }
}
