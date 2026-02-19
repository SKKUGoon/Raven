mod cli;
mod interactive;
mod ops;

use clap::Parser;
use cli::{Cli, Commands, GraphScope};
use interactive::{resolve_pipeline_inputs, PipelineInputResolver};
use raven::config::Settings;
use raven::pipeline::render::{self, GraphFormat};
use raven::pipeline::spec::PipelineSpec;
use raven::proto::control_client::ControlClient;
use raven::proto::{ListRequest, StopAllRequest};
use raven::utils::status::check_status;
use raven::utils::tree::show_users_tree;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RavenCtlPersistedConfig {
    config_file: String,
    #[serde(default)]
    run_mode: Option<String>,
}

fn persisted_config_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".raven")
            .join("ravenctl_config.json"),
    )
}

fn load_persisted_config() -> Option<RavenCtlPersistedConfig> {
    let path = persisted_config_path()?;
    let s = fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_persisted_config(cfg: &RavenCtlPersistedConfig) -> Result<PathBuf, std::io::Error> {
    let path = persisted_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(cfg).unwrap())?;
    Ok(path)
}

fn apply_persisted_env_if_missing() {
    // Allow explicit env vars to win.
    if env::var("RAVEN_CONFIG_FILE").is_ok() && env::var("RUN_MODE").is_ok() {
        return;
    }
    let Some(cfg) = load_persisted_config() else {
        return;
    };
    if env::var("RAVEN_CONFIG_FILE").is_err() && !cfg.config_file.trim().is_empty() {
        env::set_var("RAVEN_CONFIG_FILE", cfg.config_file);
    }
    if env::var("RUN_MODE").is_err() {
        if let Some(run_mode) = cfg.run_mode {
            if !run_mode.trim().is_empty() {
                env::set_var("RUN_MODE", run_mode);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle `setup` before any config is loaded.
    if let Commands::Setup { config, run_mode } = &cli.command {
        let cfg = RavenCtlPersistedConfig {
            config_file: config.clone(),
            run_mode: run_mode.clone(),
        };
        match write_persisted_config(&cfg) {
            Ok(path) => {
                println!("Saved ravenctl config to {path:?}");
                println!("This will set RAVEN_CONFIG_FILE on future `ravenctl` runs.");
            }
            Err(e) => {
                eprintln!("Failed to save ravenctl config: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // If the user ran `ravenctl setup`, use it automatically (unless env vars already set).
    apply_persisted_env_if_missing();

    // Graph can render collection topology without config; service topology is best-effort and
    // falls back to embedded defaults if loading the configured file fails.
    if let Commands::Graph { format, scope } = &cli.command {
        let fmt = GraphFormat::parse(format).unwrap_or(GraphFormat::Ascii);
        let pipeline = PipelineSpec::default();
        let settings = Settings::new().ok();
        let (show_pipeline, show_services) = match *scope {
            GraphScope::Pipeline => (true, false),
            GraphScope::Services => (false, true),
            GraphScope::All => (true, true),
        };
        let s = match fmt {
            GraphFormat::Ascii => render::render_ascii_with_scope(
                &pipeline,
                settings.as_ref(),
                show_pipeline,
                show_services,
            ),
            GraphFormat::Dot => render::render_dot_with_scope(
                &pipeline,
                settings.as_ref(),
                show_pipeline,
                show_services,
            ),
        };
        if settings.is_none() && matches!(*scope, GraphScope::Services | GraphScope::All) {
            eprintln!(
                "Warning: failed to load runtime config; service graph uses embedded default ports."
            );
        }
        print!("{s}");
        return Ok(());
    }

    let settings = Settings::new().unwrap_or_else(|e| {
        eprintln!("Failed to load config: {e}.");
        std::process::exit(1);
    });
    let service_opt = cli.service.clone();

    // Handle commands that don't need a specific client connection first
    match &cli.command {
        Commands::Status => {
            check_status(&settings).await;
            return Ok(());
        }
        Commands::User => {
            show_users_tree(&settings).await;
            return Ok(());
        }
        Commands::Plan {
            coin,
            quote,
            venue,
            venue_include,
            venue_exclude,
            interactive,
        } => {
            let inputs = resolve_pipeline_inputs(
                PipelineInputResolver::new(&settings, "plan")
                    .coin(coin.clone())
                    .quote(quote.clone())
                    .venue(venue.clone())
                    .venue_include(venue_include.clone())
                    .venue_exclude(venue_exclude.clone())
                    .interactive(*interactive),
            )?;
            let s = ops::handle_plan(
                &settings,
                &inputs.coin,
                &inputs.quote,
                &inputs.venue,
                &inputs.venue_include,
                &inputs.venue_exclude,
            )
            .await?;
            print!("{s}");
            return Ok(());
        }
        Commands::Start => {
            ops::handle_start_services(&settings).await?;
            return Ok(());
        }
        Commands::Collect {
            coin,
            quote,
            venue,
            venue_include,
            venue_exclude,
            interactive,
            print_graph,
        } => {
            if *print_graph {
                let spec = PipelineSpec::default();
                print!("{}", render::render_ascii(&spec));
            }
            let inputs = resolve_pipeline_inputs(
                PipelineInputResolver::new(&settings, "collect")
                    .coin(coin.clone())
                    .quote(quote.clone())
                    .venue(venue.clone())
                    .venue_include(venue_include.clone())
                    .venue_exclude(venue_exclude.clone())
                    .interactive(*interactive),
            )?;
            ops::handle_collect(
                &settings,
                &inputs.coin,
                &inputs.quote,
                &inputs.venue,
                &inputs.venue_include,
                &inputs.venue_exclude,
            )
            .await?;
            return Ok(());
        }
        Commands::StopAll => {
            // If no explicit --service is set, StopAll is cluster-wide and doesn't rely on a single control-plane host.
            if service_opt.is_none() {
                ops::stop_all_collections_cluster(&settings).await;
                return Ok(());
            }
        }
        Commands::Shutdown => {
            ops::shutdown(&settings, &service_opt);
            return Ok(());
        }
        Commands::List => {
            if service_opt.is_none() {
                ops::list_collections_cluster(&settings).await;
                return Ok(());
            }
            if matches!(
                service_opt.as_deref(),
                Some("binance_futures_klines" | "kline_persistence")
            ) {
                println!("List excludes kline collections; choose another service.");
                return Ok(());
            }
        }
        _ => {}
    }

    match cli.command {
        // Start/Collect are handled above
        Commands::Start => unreachable!(),
        Commands::Collect { .. } => unreachable!(),
        Commands::Plan { .. } => unreachable!(),
        Commands::Setup { .. } => unreachable!(),

        Commands::Stop {
            coin,
            quote,
            venue,
            venue_include,
            venue_exclude,
            interactive,
        } => {
            let inputs = resolve_pipeline_inputs(
                PipelineInputResolver::new(&settings, "stop")
                    .coin(coin)
                    .quote(quote)
                    .venue(venue)
                    .venue_include(venue_include)
                    .venue_exclude(venue_exclude)
                    .interactive(interactive),
            )?;
            ops::handle_stop(
                &settings,
                inputs.coin,
                inputs.quote,
                inputs.venue,
                inputs.venue_include,
                inputs.venue_exclude,
            )
            .await?;
        }
        Commands::StopAll => {
            let host = ops::resolve_control_host(cli.host, &cli.service, &settings);
            println!("Connecting to {host}");
            let mut client = ControlClient::connect(host).await?;
            let response = client
                .stop_all_collections(StopAllRequest {})
                .await?
                .into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::List => {
            let host = ops::resolve_control_host(cli.host, &cli.service, &settings);
            println!("Connecting to {host}");
            let mut client = ControlClient::connect(host).await?;
            let response = client.list_collections(ListRequest {}).await?.into_inner();
            println!("Active Collections:");
            for collection in response.collections {
                println!(
                    "- Symbol: {}, Status: {}, Subscribers: {}",
                    collection.symbol, collection.status, collection.subscriber_count
                );
            }
        }
        _ => unreachable!(), // Handled above
    }

    Ok(())
}
