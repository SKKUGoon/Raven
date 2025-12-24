use crate::config::Settings;
use crate::proto::control_client::ControlClient;
use crate::proto::ListRequest;
use crate::utils::service_registry;

pub async fn check_status(settings: &Settings) {
    let services = service_registry::all_services(settings);
    let host = service_registry::client_host(&settings.server.host);

    println!("{:<20} | {:<10} | {:<10}", "Service", "Port", "Status");
    println!("{:-<20}-|-{:-<10}-|-{:-<10}", "", "", "");

    for svc in services {
        let addr = svc.addr(host);
        let status = match ControlClient::connect(addr).await {
            Ok(mut client) => {
                match client.list_collections(ListRequest {}).await {
                    Ok(_) => "\x1b[32mHEALTHY\x1b[0m",    // Green
                    Err(_) => "\x1b[31mUNHEALTHY\x1b[0m", // Red
                }
            }
            Err(_) => "\x1b[31mUNHEALTHY\x1b[0m", // Red
        };
        println!("{:<20} | {:<10} | {}", svc.display_name, svc.port, status);
    }
}

