#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::db::timescale;
use raven::service::RavenService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_bar_persistence
    )
    .parse()?;

    // This service connects to aggregators (timebar, tibs)
    let timebar_upstreams = vec![
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_timebar_minutes
        ),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_timebar_seconds
        ),
    ];

    // This service connects to aggregators (timebar, tibs, trbs)
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

    let service_impl = timescale::new(
        timebar_upstreams,
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
