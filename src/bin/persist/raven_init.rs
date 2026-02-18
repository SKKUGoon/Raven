#[path = "../common/mod.rs"]
mod common;
mod dependency_check;
mod init_seed;

use raven::config::Settings;
use raven::db::timescale;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;
    common::init_logging(&settings);
    dependency_check::ensure_raven_init_dependencies(&settings)
        .await
        .map_err(std::io::Error::other)?;

    let seed = init_seed::collect_raven_init_seed(&settings).await;
    timescale::run_raven_init(&settings.timescale, seed).await?;
    tracing::info!("Raven init process finished successfully");
    Ok(())
}
