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
    pub host: Option<String>,
    pub port: Option<u16>,
    pub log_level: Option<String>,
    pub database_url: Option<String>,
    pub max_connections: Option<usize>,
    pub validate_only: bool,
    pub print_config: bool,
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
            Arg::new("host")
                .short('H')
                .long("host")
                .value_name("HOST")
                .help("Server host address")
                .long_help("Override the server host address from configuration")
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Server port number")
                .long_help("Override the server port number from configuration")
                .value_parser(clap::value_parser!(u16))
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .long_help("Override the log level from configuration")
                .value_parser(["trace", "debug", "info", "warn", "error"])
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .value_name("URL")
                .help("InfluxDB connection URL")
                .long_help("Override the InfluxDB connection URL from configuration")
        )
        .arg(
            Arg::new("max-connections")
                .short('m')
                .long("max-connections")
                .value_name("COUNT")
                .help("Maximum concurrent client connections")
                .long_help("Override the maximum concurrent client connections from configuration")
                .value_parser(clap::value_parser!(usize))
        )
        .arg(
            Arg::new("validate")
                .long("validate")
                .help("Validate configuration and exit")
                .long_help("Load and validate the configuration, then exit without starting the server")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("print-config")
                .long("print-config")
                .help("Print loaded configuration and exit")
                .long_help("Load configuration, print it in a readable format, then exit")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    CliArgs {
        config_file: matches.get_one::<String>("config").cloned(),
        host: matches.get_one::<String>("host").cloned(),
        port: matches.get_one::<u16>("port").copied(),
        log_level: matches.get_one::<String>("log-level").cloned(),
        database_url: matches.get_one::<String>("database-url").cloned(),
        max_connections: matches.get_one::<usize>("max-connections").copied(),
        validate_only: matches.get_flag("validate"),
        print_config: matches.get_flag("print-config"),
    }
}

/// Apply CLI overrides to configuration
pub fn apply_cli_overrides(mut config: RuntimeConfig, args: &CliArgs) -> RuntimeConfig {
    if let Some(host) = &args.host {
        info!("CLI override: host = {}", host);
        config.server.host = host.clone();
    }

    if let Some(port) = args.port {
        info!("CLI override: port = {}", port);
        config.server.port = port;
    }

    if let Some(log_level) = &args.log_level {
        info!("CLI override: log_level = {}", log_level);
        config.monitoring.log_level = log_level.clone();
    }

    if let Some(database_url) = &args.database_url {
        info!("CLI override: database_url = {}", database_url);
        config.database.influx_url = database_url.clone();
    }

    if let Some(max_connections) = args.max_connections {
        info!("CLI override: max_connections = {}", max_connections);
        config.server.max_connections = max_connections;
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
