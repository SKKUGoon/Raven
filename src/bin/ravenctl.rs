#[path = "ravenctl/cli.rs"]
mod cli;
#[path = "ravenctl/ops.rs"]
mod ops;

use clap::Parser;
use cli::{Cli, Commands};
use raven::config::Settings;
use raven::pipeline::render::{self, GraphFormat};
use raven::pipeline::spec::PipelineSpec;
use raven::proto::control_client::ControlClient;
use raven::proto::{ListRequest, StopAllRequest};
use raven::utils::status::check_status;
use raven::utils::tree::show_users_tree;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Graph should not require config; it only depends on the compiled-in PipelineSpec.
    if let Commands::Graph { format } = &cli.command {
        let spec = PipelineSpec::default();
        let fmt = GraphFormat::parse(format).unwrap_or(GraphFormat::Ascii);
        let s = match fmt {
            GraphFormat::Ascii => render::render_ascii(&spec),
            GraphFormat::Dot => render::render_dot(&spec),
        };
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
            symbol,
            base,
            venue,
            venue_include,
            venue_exclude,
        } => {
            let s = ops::handle_plan(&settings, symbol, base, venue, venue_include, venue_exclude)
                .await?;
            print!("{s}");
            return Ok(());
        }
        Commands::Start {
            symbol,
            base,
            venue,
            venue_include,
            venue_exclude,
            print_graph,
        } => {
            if *print_graph {
                let spec = PipelineSpec::default();
                print!("{}", render::render_ascii(&spec));
            }
            ops::handle_start(
                &settings,
                symbol,
                base,
                venue,
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
        Commands::Plan { .. } => unreachable!(),

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
