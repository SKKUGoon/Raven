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
