use clap::{Parser, Subcommand, ValueEnum};
use raven::utils::service_registry;

fn parse_service_id(s: &str) -> Result<String, String> {
    let normalized = match s {
        // Backward-compatible alias used by older scripts.
        "persistence" => "tick_persistence",
        _ => s,
    };
    if service_registry::is_known_service_id(normalized) {
        return Ok(normalized.to_string());
    }
    Err(format!(
        "unknown service `{s}` (expected one of: {})",
        service_registry::KNOWN_SERVICE_IDS.join(", ")
    ))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum GraphScope {
    /// Collection-control pipeline graph only.
    Pipeline,
    /// Process topology graph for all known microservices.
    Services,
    /// Print both pipeline and process topology.
    All,
}

#[derive(Parser)]
#[command(name = "ravenctl")]
#[command(about = "Control Raven microservices", long_about = None)]
pub struct Cli {
    #[arg(long, default_value = "http://localhost:50051")]
    pub host: String,

    /// Target service id for service-scoped commands.
    ///
    /// Run `ravenctl graph --scope services` to see the full service map.
    #[arg(short, long, value_parser = parse_service_id)]
    pub service: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Persist config location for `ravenctl` (used by all commands and spawned services).
    ///
    /// This writes a small file under `~/.raven/` and, on subsequent runs, `ravenctl` will
    /// automatically set `RAVEN_CONFIG_FILE` (and optionally `RUN_MODE`) for itself and any
    /// services it spawns.
    Setup {
        /// Path to a config file like `prod.toml` (absolute or relative).
        #[arg(long)]
        config: String,
        /// Optional: persist RUN_MODE as well (e.g. prod / test).
        #[arg(long)]
        run_mode: Option<String>,
    },

    /// Start all registered microservice processes (no collection-control calls).
    Start,

    /// Print collection-control plan for a coin/venue selection (no execution).
    Plan {
        /// Coin (e.g. ETH) or venue symbol (e.g. ETHUSDC).
        ///
        /// - If you pass `--quote`, this is interpreted as the coin (e.g. ETH).
        /// - If you omit `--quote`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short = 'c', long)]
        coin: Option<String>,
        /// Quote currency (e.g. USDC). If provided, `--coin` is treated as a coin code.
        #[arg(short = 'q', long)]
        quote: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
        /// Prompt for missing inputs and venue selection.
        #[arg(short = 'i', long)]
        interactive: bool,
    },
    /// Start collection-control streams for a coin/venue selection.
    Collect {
        /// Coin (e.g. ETH) or venue symbol (e.g. ETHUSDC).
        ///
        /// - If you pass `--quote`, this is interpreted as the coin (e.g. ETH).
        /// - If you omit `--quote`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short = 'c', long)]
        coin: Option<String>,
        /// Quote currency (e.g. USDC). If provided, `--coin` is treated as a coin code.
        #[arg(short = 'q', long)]
        quote: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
        /// Prompt for missing inputs and venue selection.
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Print the pipeline graph before executing.
        #[arg(long)]
        print_graph: bool,
    },
    /// Stop collection-control streams for a coin/venue selection.
    Stop {
        /// Coin (e.g. ETH) or venue symbol (e.g. ETHUSDC).
        ///
        /// - If you pass `--quote`, this is interpreted as the coin (e.g. ETH).
        /// - If you omit `--quote`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short = 'c', long)]
        coin: Option<String>,
        /// Quote currency (e.g. USDC). If provided, `--coin` is treated as a coin code.
        #[arg(short = 'q', long)]
        quote: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
        /// Prompt for missing inputs and venue selection.
        #[arg(short = 'i', long)]
        interactive: bool,
    },
    /// Stop all collections across services (or only --service).
    StopAll,
    /// Shutdown service processes (all services, or only --service).
    Shutdown,
    /// List active collections across services (or only --service).
    List,
    /// Check runtime status of all registered services.
    Status,
    /// Show service users/subscriptions tree
    User,

    /// Print graph topology (collection pipeline and/or service processes).
    Graph {
        /// Output format: ascii | dot
        #[arg(long, default_value = "ascii")]
        format: String,
        /// Graph scope: pipeline | services | all
        #[arg(long, value_enum, default_value_t = GraphScope::All)]
        scope: GraphScope,
    },
}
