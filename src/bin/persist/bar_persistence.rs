#[path = "../common/mod.rs"]
mod common;
mod init_seed;

use raven::config::Settings;
use raven::db::timescale;
use raven::service::RavenService;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_bar_persistence
    )
    .parse()?;

    let tibs_upstreams = vec![
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_tibs_small
        ),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_tibs_large
        ),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_trbs_small
        ),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_trbs_large
        ),
    ];

    let vibs_upstreams = vec![
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_vibs_small
        ),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_vibs_large
        ),
    ];

    let vpin_upstreams = vec![format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_vpin
    )];

    let init_seed = init_seed::collect_raven_init_seed(&settings).await;
    if let Err(e) = timescale::run_raven_init(&settings.timescale, init_seed).await {
        warn!("Raven init failed before bar persistence startup: {}", e);
    } else {
        info!("Raven init completed before bar persistence startup");
    }

    let service_impl = timescale::new(
        tibs_upstreams,
        vibs_upstreams,
        vpin_upstreams,
        settings.timescale.clone(),
    )
    .await?;
    let raven = RavenService::new("BarPersistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
