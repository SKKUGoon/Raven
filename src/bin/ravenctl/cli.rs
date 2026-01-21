use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ravenctl")]
#[command(about = "Control Raven services", long_about = None)]
pub struct Cli {
    #[arg(long, default_value = "http://localhost:50051")]
    pub host: String,

    /// Target service: binance_spot, binance_futures, binance_futures_klines, tick_persistence, bar_persistence, kline_persistence, timebar_60s, timebar_1s, tibs_small, tibs_large, trbs_small, trbs_large, vibs_small, vibs_large, vpin
    #[arg(short, long)]
    pub service: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Persist the config file location for `ravenctl` (so it can be run from any directory).
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

    /// Start services (no args) OR Start data collection for a symbol (with args)
    Start {
        /// Symbol or base asset to collect.
        ///
        /// - If you pass `--base`, this is interpreted as the base asset (e.g. ETH).
        /// - If you omit `--base`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short, long)]
        symbol: Option<String>,
        /// Quote / base currency (e.g. USDC). If provided, `--symbol` is treated as the base asset.
        #[arg(long)]
        base: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,

        /// Print the pipeline graph before executing.
        #[arg(long)]
        print_graph: bool,
    },

    /// Print the execution plan for starting a symbol (no execution).
    Plan {
        /// Symbol or base asset to collect.
        ///
        /// - If you pass `--base`, this is interpreted as the base asset (e.g. ETH).
        /// - If you omit `--base`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short, long)]
        symbol: String,
        /// Quote / base currency (e.g. USDC). If provided, `--symbol` is treated as the base asset.
        #[arg(long)]
        base: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
    },
    /// Stop data collection for a symbol
    Stop {
        /// Symbol or base asset to stop.
        ///
        /// - If you pass `--base`, this is interpreted as the base asset (e.g. ETH).
        /// - If you omit `--base`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short, long)]
        symbol: String,
        /// Quote / base currency (e.g. USDC). If provided, `--symbol` is treated as the base asset.
        #[arg(long)]
        base: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
    },
    /// Stop all data collections (all services, unless --service is set)
    StopAll,
    /// Shutdown Raven services (all services, unless --service is set)
    Shutdown,
    /// List active collections
    List,
    /// Check status of all services
    Status,
    /// Show service users/subscriptions tree
    User,

    /// Print the pipeline graph (topology), optionally as DOT for Graphviz.
    Graph {
        /// Output format: ascii | dot
        #[arg(long, default_value = "ascii")]
        format: String,
    },
}
