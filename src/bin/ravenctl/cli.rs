use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ravenctl")]
#[command(about = "Control Raven services", long_about = None)]
pub struct Cli {
    #[arg(long, default_value = "http://localhost:50051")]
    pub host: String,

    /// Target service: binance_spot, binance_futures, tick_persistence, bar_persistence, timebar_60s, timebar_1s, tibs_small, tibs_large, trbs_small, trbs_large, vibs_small, vibs_large, vpin
    #[arg(short, long)]
    pub service: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
        exchange: Option<String>,
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
    /// Alias for Status
    Ping,
    /// Show service users/subscriptions tree
    User,
}
