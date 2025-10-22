use serde::de::DeserializeOwned;

use crate::error::RavenResult;

/// Trait implemented by individual configuration sections.
///
/// Each section is responsible for providing validation logic and a key that
/// matches the section name in the TOML configuration files.
pub trait ConfigSection: DeserializeOwned + Default + Send + Sync {
    /// Top-level key for the section inside the configuration file.
    const KEY: &'static str;

    /// Validate semantic correctness of the section.
    fn validate(&self) -> RavenResult<()>;
}
