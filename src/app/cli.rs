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
        .get_matches();

    CliArgs {
        config_file: matches.get_one::<String>("config").cloned(),
    }
}

/// Apply CLI overrides to configuration
pub fn apply_cli_overrides(mut config: RuntimeConfig, args: &CliArgs) -> RuntimeConfig {
    if let Some(path) = &args.config_file {
        info!("Using configuration override from CLI: {}", path);
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
