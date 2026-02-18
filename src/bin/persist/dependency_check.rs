use raven::config::Settings;
use raven::utils::service_registry;
use sqlx::postgres::PgPoolOptions;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tracing::{info, warn};
use url::Url;

#[derive(Debug)]
struct MissingDependency {
    name: String,
    detail: String,
}

pub async fn ensure_raven_init_dependencies(settings: &Settings) -> Result<(), String> {
    info!("Running raven_init dependency checks...");

    let mut missing: Vec<MissingDependency> = Vec::new();

    check_timescale_connectivity(settings, &mut missing).await;
    check_timescaledb_extension(settings, &mut missing).await;
    check_service_port_collisions(settings, &mut missing).await;
    check_optional_host_tools();
    check_optional_binance_api().await;
    report_influx_context();

    if missing.is_empty() {
        info!("Dependency checks passed.");
        return Ok(());
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push("Missing dependencies detected for raven_init:".to_string());
    for item in &missing {
        lines.push(format!("- {}: {}", item.name, item.detail));
    }
    lines.push("Resolve the items above, then rerun `raven_init`.".to_string());
    Err(lines.join("\n"))
}

async fn check_timescale_connectivity(settings: &Settings, missing: &mut Vec<MissingDependency>) {
    let parsed = match Url::parse(&settings.timescale.url) {
        Ok(u) => u,
        Err(e) => {
            missing.push(MissingDependency {
                name: "timescale.url".to_string(),
                detail: format!("invalid PostgreSQL URL format ({e})"),
            });
            return;
        }
    };

    let host = parsed.host_str().map(str::to_string).unwrap_or_default();
    let port = parsed.port_or_known_default().unwrap_or(5432);
    if host.is_empty() {
        missing.push(MissingDependency {
            name: "Timescale host".to_string(),
            detail: "timescale.url does not include a hostname".to_string(),
        });
        return;
    }

    match timeout(Duration::from_secs(3), TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(_)) => {
            info!("Timescale TCP reachable at {host}:{port}");
        }
        Ok(Err(e)) => {
            missing.push(MissingDependency {
                name: "Timescale TCP reachability".to_string(),
                detail: format!(
                    "cannot connect to {host}:{port} ({e}); ensure PostgreSQL/TimescaleDB is running and network allows this host/port"
                ),
            });
            return;
        }
        Err(_) => {
            missing.push(MissingDependency {
                name: "Timescale TCP reachability".to_string(),
                detail: format!(
                    "connection timed out to {host}:{port}; ensure PostgreSQL/TimescaleDB is running and reachable"
                ),
            });
            return;
        }
    }

    let connect = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&settings.timescale.url)
        .await;
    if let Err(e) = connect {
        missing.push(MissingDependency {
            name: "Timescale authentication".to_string(),
            detail: format!(
                "database login/query failed ({e}); check username/password/database in timescale.url"
            ),
        });
    }
}

async fn check_timescaledb_extension(settings: &Settings, missing: &mut Vec<MissingDependency>) {
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&settings.timescale.url)
        .await
    {
        Ok(pool) => pool,
        Err(_) => return,
    };

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb')",
    )
    .fetch_one(&pool)
    .await;

    match exists {
        Ok(true) => info!("TimescaleDB extension is installed."),
        Ok(false) => missing.push(MissingDependency {
            name: "TimescaleDB extension".to_string(),
            detail: "extension `timescaledb` is not installed/enabled in target database".to_string(),
        }),
        Err(e) => missing.push(MissingDependency {
            name: "TimescaleDB extension check".to_string(),
            detail: format!("failed to query pg_extension ({e})"),
        }),
    }
}

async fn check_service_port_collisions(settings: &Settings, missing: &mut Vec<MissingDependency>) {
    let host = settings.server.host.trim();
    if host.is_empty() {
        missing.push(MissingDependency {
            name: "server.host".to_string(),
            detail: "server.host is empty".to_string(),
        });
        return;
    }

    let mut by_port: BTreeMap<u16, Vec<&str>> = BTreeMap::new();
    for svc in service_registry::all_services(settings) {
        by_port.entry(svc.port).or_default().push(svc.id);
    }

    for (port, ids) in &by_port {
        if ids.len() > 1 {
            missing.push(MissingDependency {
                name: format!("Port assignment conflict ({port})"),
                detail: format!("multiple services share this port: {}", ids.join(", ")),
            });
        }
    }

    let bind_host = if host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        host
    };

    let mut reported: BTreeSet<u16> = BTreeSet::new();
    for svc in service_registry::all_services(settings) {
        if !reported.insert(svc.port) {
            continue;
        }
        let addr = format!("{bind_host}:{}", svc.port);
        match TcpListener::bind(&addr).await {
            Ok(listener) => drop(listener),
            Err(e) if e.kind() == ErrorKind::AddrInUse => {
                missing.push(MissingDependency {
                    name: format!("Port in use ({})", svc.port),
                    detail: format!(
                        "{addr} is already occupied; stop the process using this port before `ravenctl start`"
                    ),
                });
            }
            Err(e) if e.kind() == ErrorKind::AddrNotAvailable => {
                missing.push(MissingDependency {
                    name: format!("Bind host unavailable ({})", svc.port),
                    detail: format!(
                        "cannot bind to {addr} ({e}); check `server.host` and local network interfaces"
                    ),
                });
            }
            Err(e) => {
                warn!("Port probe skipped for {addr}: {e}");
            }
        }
    }
}

fn check_optional_host_tools() {
    if binary_in_path("lsof") {
        info!("Optional host tool found: lsof");
    } else {
        warn!(
            "Optional host tool missing: lsof (raven_init can still run; `ravenctl shutdown` port-based fallback may be limited)"
        );
    }
}

async fn check_optional_binance_api() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build();
    let Ok(client) = client else {
        warn!("Skipped optional Binance reachability check (failed to create HTTP client).");
        return;
    };

    let ping = client
        .get("https://fapi.binance.com/fapi/v1/ping")
        .send()
        .await;
    if ping.is_err() {
        warn!(
            "Optional network dependency unavailable: cannot reach Binance Futures REST. raven_init will still proceed with config-derived symbols."
        );
    }
}

fn report_influx_context() {
    info!("InfluxDB is not required for raven_init. It is required later for tick_persistence.");
}

fn binary_in_path(bin: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| has_executable_in_dir(&dir, bin))
}

fn has_executable_in_dir(dir: &Path, bin: &str) -> bool {
    let candidate = dir.join(bin);
    if candidate.exists() {
        return true;
    }

    #[cfg(windows)]
    {
        let exe = dir.join(format!("{bin}.exe"));
        if exe.exists() {
            return true;
        }
    }
    false
}
