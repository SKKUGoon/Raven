use raven::config::Settings;
use raven::utils::process::{stop_all_services, stop_service};
use raven::utils::service_registry;

pub fn shutdown(settings: &Settings, service_opt: &Option<String>) {
    if let Some(svc_id) = service_opt.as_deref() {
        let services = service_registry::all_services(settings);
        if let Some(spec) = services.iter().find(|svc| svc.id == svc_id) {
            if stop_service(spec.log_name) {
                println!("Shutdown requested for {}", spec.display_name);
            } else {
                eprintln!(
                    "No pid file found for {} ({}). Did you start it via `ravenctl start`?",
                    spec.display_name, spec.log_name
                );
            }
        } else {
            eprintln!("Unknown service id: {svc_id}");
        }
    } else {
        stop_all_services();
    }
}

pub fn resolve_control_host(
    cli_host: String,
    service_opt: &Option<String>,
    settings: &Settings,
) -> String {
    if let Some(s) = service_opt.as_deref() {
        let host_ip = &settings.server.host;
        // Resolve against our canonical service registry first.
        let services = service_registry::all_services(settings);
        if let Some(spec) = services.iter().find(|svc| svc.id == s) {
            spec.addr(host_ip)
        } else {
            match s {
                "persistence" => format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_tick_persistence
                ),
                "timebar" | "timebar_minutes" => {
                    format!(
                        "http://{}:{}",
                        host_ip, settings.server.port_timebar_minutes
                    )
                }
                "timebar_seconds" => {
                    format!(
                        "http://{}:{}",
                        host_ip, settings.server.port_timebar_seconds
                    )
                }
                _ => {
                    eprintln!("Unknown service: {s}. Using default host.");
                    cli_host
                }
            }
        }
    } else {
        cli_host
    }
}


