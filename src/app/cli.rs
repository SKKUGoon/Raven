use crate::config::RuntimeConfig;
use clap::{Arg, Command};
use tracing::info;

/// Application version information
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
const GIT_SHA: &str = env!("VERGEN_GIT_SHA");

/// CLI arguments structure
#[derive(Debug)]
pub struct CliArgs {
    pub config_file: Option<String>,
    pub symbols: Vec<String>,
}

/// Parse command line arguments
pub fn parse_cli_args() -> CliArgs {
    let matches = Command::new("Project Raven")
        .version(VERSION)
        .author("The Crow's Watch")
        .about("High-performance market data subscription server")
        .long_about(format!(
            "Project Raven - Market Data Subscription Server\n\
             Version: {VERSION}\n\
             Build: {BUILD_TIMESTAMP}\n\
             Git SHA: {GIT_SHA}\n\n\
             A high-performance gRPC-based market data distribution system\n\
             designed for financial trading applications."
        ))
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .long_help("Path to the configuration file. Defaults to the environment-specific file (config/development.toml or config/secret.toml)")
        )
        .arg(
            Arg::new("symbols")
                .long("symbols")
                .value_name("SYMBOLS")
                .help("Comma-separated list of Binance futures symbols to stream (max 10, e.g. BTCUSDT,ETHUSDT)")
                .long_help("Provide up to 10 Binance futures symbols. Default is BTCUSDT if not specified.")
                .value_delimiter(',')
                .num_args(1..=10),
        )
        .get_matches();

    let symbols = matches
        .get_many::<String>("symbols")
        .map(|vals| {
            vals.into_iter()
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    CliArgs {
        config_file: matches.get_one::<String>("config").cloned(),
        symbols,
    }
}

/// Apply CLI overrides to configuration
pub fn apply_cli_overrides(config: RuntimeConfig, args: &CliArgs) -> RuntimeConfig {
    if let Some(path) = &args.config_file {
        info!("Using configuration override from CLI: {}", path);
    }
    if !args.symbols.is_empty() {
        info!("CLI override: symbols = {}", args.symbols.join(", "));
    }
    config
}

/// Print Raven ASCII art
pub fn print_raven_ascii_art() {
    println!(
        r#"

    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗
    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║
    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║
    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║
    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║
    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝
    "#
    );
}

/// Print application version and build information
pub fn print_version_info() {
    print_raven_ascii_art();
    println!();
    println!("Version: {VERSION}");
    println!("Build Timestamp: {BUILD_TIMESTAMP}");
    println!("Git SHA: {GIT_SHA}");
    println!("Rust Version: {}", env!("VERGEN_RUSTC_SEMVER"));
    println!("Target: {}", env!("VERGEN_CARGO_TARGET_TRIPLE"));
    println!();
}
