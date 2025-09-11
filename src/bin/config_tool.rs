// Configuration Management Tool - Project Raven
// "A tool to shape the realm's destiny"

use anyhow::Result;
use clap::{Arg, Command};
use market_data_subscription_server::config::{Config, ConfigManager, ConfigUtils};
use std::time::Duration;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let matches = Command::new("raven-config")
        .version("1.0")
        .about("Project Raven Configuration Management Tool")
        .subcommand(
            Command::new("validate")
                .about("Validate current configuration")
                .arg(
                    Arg::new("config-path")
                        .short('c')
                        .long("config")
                        .value_name("PATH")
                        .help("Path to configuration file"),
                ),
        )
        .subcommand(
            Command::new("show")
                .about("Show current configuration")
                .arg(
                    Arg::new("format")
                        .short('f')
                        .long("format")
                        .value_name("FORMAT")
                        .help("Output format (pretty, json)")
                        .default_value("pretty"),
                ),
        )
        .subcommand(Command::new("health").about("Check configuration health and show warnings"))
        .subcommand(
            Command::new("template")
                .about("Generate configuration template")
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("FILE")
                        .help("Output file path"),
                ),
        )
        .subcommand(Command::new("env-check").about("Check environment variables"))
        .subcommand(
            Command::new("recommendations")
                .about("Show environment-specific recommendations")
                .arg(
                    Arg::new("environment")
                        .short('e')
                        .long("env")
                        .value_name("ENV")
                        .help("Environment (development, production, staging)")
                        .default_value("development"),
                ),
        )
        .subcommand(
            Command::new("watch")
                .about("Watch configuration file for changes")
                .arg(
                    Arg::new("config-path")
                        .short('c')
                        .long("config")
                        .value_name("PATH")
                        .help("Path to configuration file")
                        .default_value("config/default.toml"),
                )
                .arg(
                    Arg::new("interval")
                        .short('i')
                        .long("interval")
                        .value_name("SECONDS")
                        .help("Check interval in seconds")
                        .default_value("5"),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("validate", sub_matches)) => {
            validate_config(sub_matches.get_one::<String>("config-path")).await?;
        }
        Some(("show", sub_matches)) => {
            show_config(sub_matches.get_one::<String>("format").unwrap()).await?;
        }
        Some(("health", _)) => {
            check_health().await?;
        }
        Some(("template", sub_matches)) => {
            generate_template(sub_matches.get_one::<String>("output"))?;
        }
        Some(("env-check", _)) => {
            check_environment()?;
        }
        Some(("recommendations", sub_matches)) => {
            show_recommendations(sub_matches.get_one::<String>("environment").unwrap())?;
        }
        Some(("watch", sub_matches)) => {
            let config_path = sub_matches.get_one::<String>("config-path").unwrap();
            let interval = sub_matches
                .get_one::<String>("interval")
                .unwrap()
                .parse::<u64>()
                .unwrap_or(5);
            watch_config(config_path, interval).await?;
        }
        _ => {
            println!("Use --help to see available commands");
        }
    }

    Ok(())
}

async fn validate_config(_config_path: Option<&String>) -> Result<()> {
    info!("🔍 Validating configuration...");

    match Config::load() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");

            match config.validate() {
                Ok(_) => {
                    info!("✅ Configuration validation passed");
                    ConfigUtils::print_config(&config);
                }
                Err(e) => {
                    error!("❌ Configuration validation failed: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn show_config(format: &str) -> Result<()> {
    let config = Config::load()?;

    match format {
        "json" => {
            let json = ConfigUtils::export_as_json(&config)?;
            println!("{json}");
        }
        "pretty" => {
            ConfigUtils::print_config(&config);
        }
        _ => {
            ConfigUtils::print_config(&config);
        }
    }

    Ok(())
}

async fn check_health() -> Result<()> {
    info!("🏥 Checking configuration health...");

    let config = Config::load()?;
    let warnings = ConfigUtils::check_configuration_health(&config);

    if warnings.is_empty() {
        info!("✅ No configuration health issues found");
    } else {
        info!("⚠️  Found {} configuration warnings:", warnings.len());
        for (i, warning) in warnings.iter().enumerate() {
            info!("  {}. {}", i + 1, warning);
        }
    }

    let summary = ConfigUtils::get_config_summary(&config);
    info!("📊 Configuration Summary:");
    for (key, value) in summary {
        info!("  {}: {}", key, value);
    }

    Ok(())
}

fn generate_template(output_path: Option<&String>) -> Result<()> {
    let template = ConfigUtils::generate_config_template();

    match output_path {
        Some(path) => {
            std::fs::write(path, template)?;
            info!("✅ Configuration template written to: {}", path);
        }
        None => {
            println!("{template}");
        }
    }

    Ok(())
}

fn check_environment() -> Result<()> {
    info!("🌍 Checking environment variables...");
    ConfigUtils::validate_environment()?;
    info!("✅ Environment check completed");
    Ok(())
}

fn show_recommendations(environment: &str) -> Result<()> {
    let recommendations = ConfigUtils::get_environment_recommendations();

    if let Some(env_recs) = recommendations.get(environment) {
        info!("💡 Recommendations for {} environment:", environment);
        for (i, rec) in env_recs.iter().enumerate() {
            info!("  {}. {}", i + 1, rec);
        }
    } else {
        error!("❌ Unknown environment: {}", environment);
        info!("Available environments: development, production, staging");
    }

    Ok(())
}

async fn watch_config(config_path: &str, interval_seconds: u64) -> Result<()> {
    info!("👁️  Watching configuration file: {}", config_path);
    info!("🔄 Check interval: {} seconds", interval_seconds);
    info!("Press Ctrl+C to stop watching");

    let manager = ConfigManager::new(
        config_path.to_string(),
        Duration::from_secs(interval_seconds),
    )?;

    // Start hot-reload monitoring
    manager.start_hot_reload().await?;

    // Keep the program running
    tokio::signal::ctrl_c().await?;
    info!("👋 Stopping configuration watcher");

    Ok(())
}
