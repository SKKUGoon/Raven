use std::env;
use std::path::{Path, PathBuf};

use config::{Config as RawConfig, Environment, File, FileFormat};
use serde::de::DeserializeOwned;
use tracing::warn;

use crate::config::validation::ConfigSection;
use crate::error::{RavenError, RavenResult};

#[derive(Debug, Clone)]
pub struct ConfigLoader {
    environment: String,
    explicit_file: Option<PathBuf>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        Self {
            environment,
            explicit_file: None,
        }
    }
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.explicit_file = Some(path.into());
        self
    }

    pub fn environment(&self) -> &str {
        &self.environment
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        if let Some(path) = &self.explicit_file {
            Some(path.clone())
        } else {
            Some(default_path_for_environment(&self.environment))
        }
    }

    pub fn load_section<T>(&self) -> RavenResult<T>
    where
        T: ConfigSection + DeserializeOwned,
    {
        let raw = self.build()?;

        let section = match raw.get::<T>(T::KEY) {
            Ok(section) => section,
            Err(config::ConfigError::NotFound(_)) => T::default(),
            Err(e) => {
                return Err(RavenError::configuration(format!(
                    "Failed to load '{}' configuration section: {e}",
                    T::KEY
                )))
            }
        };

        section.validate()?;
        Ok(section)
    }

    pub fn load_section_from_path<T>(path: impl AsRef<Path>) -> RavenResult<T>
    where
        T: ConfigSection + DeserializeOwned,
    {
        ConfigLoader::new()
            .with_file(path.as_ref().to_path_buf())
            .load_section::<T>()
    }

    pub fn build(&self) -> RavenResult<RawConfig> {
        let mut builder = config::Config::builder();

        if let Some(path) = self.config_path() {
            if path.exists() {
                let path_str = path.to_string_lossy().into_owned();
                let file_source = File::new(&path_str, FileFormat::Toml).required(true);
                builder = builder.add_source(file_source);
            } else {
                warn!(
                    "Configuration file not found at {} – falling back to defaults and environment variables",
                    path.display()
                );
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("RAVEN")
                .prefix_separator("_")
                .separator("__"),
        );

        builder.build().map_err(|e| {
            RavenError::configuration(format!("Failed to build configuration sources: {e}"))
        })
    }
}

fn default_path_for_environment(environment: &str) -> PathBuf {
    match environment {
        "production" => PathBuf::from("config/secret.toml"),
        "staging" => PathBuf::from("config/staging.toml"),
        _ => PathBuf::from("config/development.toml"),
    }
}
