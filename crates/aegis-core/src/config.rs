//! Configuration management for Aegis-MCP
//!
//! Supports multiple configuration sources: environment variables, config files, and defaults.

use serde::{Deserialize, Serialize};
use anyhow::Result as AnyhowResult;

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 1000,
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    // Additional configuration sections will be added in subsequent issues
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from multiple sources
    pub fn load() -> AnyhowResult<Self> {
        // Placeholder: Will be fully implemented in Issue #2
        Ok(Config::default())
    }
}
