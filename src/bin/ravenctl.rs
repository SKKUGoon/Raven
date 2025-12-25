#[path = "ravenctl/cli.rs"]
mod cli;
#[path = "ravenctl/ops.rs"]
mod ops;

use clap::Parser;
use cli::{Cli, Commands};
use raven::config::Settings;
use raven::proto::control_client::ControlClient;
use raven::proto::{ListRequest, StopAllRequest};
use raven::utils::status::check_status;
use raven::utils::tree::show_users_tree;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let settings = Settings::new().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {e}. Using defaults.");
        std::process::exit(1);
    });
    let service_opt = cli.service.clone();

    // Handle commands that don't need a specific client connection first
    match &cli.command {
        Commands::Status | Commands::Ping => {
            check_status(&settings).await;
            return Ok(());
        }
        Commands::User => {
            show_users_tree(&settings).await;
            return Ok(());
        }
        Commands::Start {
            symbol,
            base,
            exchange,
            venue_include,
            venue_exclude,
        } => {
            ops::handle_start(
                &settings,
                symbol,
                base,
                exchange,
                venue_include,
                venue_exclude,
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
        _ => {}
    }

    // For other commands (Stop, StopAll, List), connect to the target host
    let host = ops::resolve_control_host(cli.host, &cli.service, &settings);

    println!("Connecting to {host}");
    // We connect lazily or just try to connect now
    let mut client = ControlClient::connect(host).await?;

    match cli.command {
        // Start is handled above
        Commands::Start { .. } => unreachable!(),

        Commands::Stop {
            symbol,
            base,
            venue,
            venue_include,
            venue_exclude,
        } => {
            // Keep the initial control-plane connect (above) for backwards compat,
            // even though Stop is implemented via per-service control endpoints.
            let _ = &mut client;
            ops::handle_stop(&settings, symbol, base, venue, venue_include, venue_exclude).await?;
        }
        Commands::StopAll => {
            let response = client
                .stop_all_collections(StopAllRequest {})
                .await?
                .into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::List => {
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
