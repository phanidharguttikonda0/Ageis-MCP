//! Configuration management for Aegis-MCP
//!
//! Supports multiple configuration sources with priority:
//! 1. Environment variables
//! 2. Config files (production.toml/development.toml)
//! 3. Default values

use serde::{Deserialize, Serialize};
use anyhow::{Result as AnyhowResult, anyhow, Context};
use config::{Config, Environment, File as ConfigFile};
use std::path::Path;

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

/// Redis configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub connection_timeout: u64,
    pub max_lifetime: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            connection_timeout: 5000,
            max_lifetime: 3600,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CacheConfig {
    pub similarity_threshold: f32,
    pub max_cache_size: usize,
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.95,
            max_cache_size: 10000,
            ttl_seconds: 86400,
        }
    }
}

/// LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LLMConfig {
    pub upstream_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: u64,
    pub max_retries: u32,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            upstream_url: "https://api.openai.com/v1".to_string(),
            api_key: "".to_string(),
            model: "gpt-4".to_string(),
            timeout: 30000,
            max_retries: 3,
        }
    }
}

/// Observability configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ObservabilityConfig {
    pub log_level: String,
    pub jaeger_endpoint: String,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            jaeger_endpoint: "http://localhost:4318".to_string(),
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub cache: CacheConfig,
    pub llm: LLMConfig,
    pub observability: ObservabilityConfig,
}

impl AppConfig {
    /// Load configuration from multiple sources with proper priority
    ///
    /// Priority order (highest to lowest):
    /// 1. Environment variables (prefix: AEGIS_)
    /// 2. Config files (config/production.toml or config/development.toml)
    /// 3. Default values
    pub fn load() -> AnyhowResult<Self> {
        Self::load_from_env("AEGIS")
    }

    /// Load configuration from specific environment prefix
    pub fn load_from_env(prefix: &str) -> AnyhowResult<Self> {
        let environment = std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string());

        let config_file = format!("config/{}.toml", environment);

        // Build configuration with proper priority
        let settings = Config::builder()
            // Add config file (optional) - specify TOML format
            .add_source(ConfigFile::new(&config_file, config::FileFormat::Toml).required(false))
            // Add environment variables
            .add_source(
                Environment::with_prefix(prefix)
                    .prefix_separator("_")
                    .separator("__")
                    .try_parsing(true)
            )
            .build()
            .context("Failed to build configuration")?;

        // Try to deserialize, fall back to defaults if config file doesn't exist
        let config: AppConfig = settings.try_deserialize().unwrap_or_default();

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration values
    fn validate(&self) -> AnyhowResult<()> {
        // Validate server configuration
        if self.server.port == 0 {
            return Err(anyhow!("Server port cannot be 0"));
        }
        if self.server.max_connections == 0 {
            return Err(anyhow!("Max connections cannot be 0"));
        }

        // Validate Redis configuration
        if self.redis.url.is_empty() {
            return Err(anyhow!("Redis URL cannot be empty"));
        }
        if self.redis.pool_size == 0 {
            return Err(anyhow!("Redis pool size cannot be 0"));
        }

        // Validate cache configuration
        if !(0.0..=1.0).contains(&self.cache.similarity_threshold) {
            return Err(anyhow!("Cache similarity threshold must be between 0.0 and 1.0"));
        }
        if self.cache.max_cache_size == 0 {
            return Err(anyhow!("Max cache size cannot be 0"));
        }

        // Validate LLM configuration
        if self.llm.upstream_url.is_empty() {
            return Err(anyhow!("LLM upstream URL cannot be empty"));
        }
        if self.llm.api_key.is_empty() {
            return Err(anyhow!("LLM API key cannot be empty. Set AEGIS_LLM__API_KEY environment variable"));
        }
        if self.llm.model.is_empty() {
            return Err(anyhow!("LLM model cannot be empty"));
        }
        if self.llm.timeout == 0 {
            return Err(anyhow!("LLM timeout cannot be 0"));
        }

        // Validate observability configuration
        let valid_log_levels = vec!["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.observability.log_level.as_str()) {
            return Err(anyhow!("Invalid log level '{}'. Must be one of: {:?}",
                self.observability.log_level, valid_log_levels));
        }

        Ok(())
    }

    /// Get redacted configuration for logging (hides secrets)
    pub fn redacted(&self) -> String {
        format!(
            "AppConfig {{\n\
             server: {:?},\n\
             redis: RedisConfig {{ url: {:?}, pool_size: {}, connection_timeout: {}, max_lifetime: {} }},\n\
             cache: {:?},\n\
             llm: LLMConfig {{ upstream_url: {:?}, api_key: \"***REDACTED***\", model: {:?}, timeout: {}, max_retries: {} }},\n\
             observability: {:?}\n\
             }}",
            self.server,
            self.redis.url, self.redis.pool_size, self.redis.connection_timeout, self.redis.max_lifetime,
            self.cache,
            self.llm.upstream_url, self.llm.model, self.llm.timeout, self.llm.max_retries,
            self.observability
        )
    }

    /// Check if config file exists at the given path
    pub fn has_config_file(path: &str) -> bool {
        Path::new(path).exists()
    }

    /// Get the current environment
    pub fn environment() -> String {
        std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.cache.similarity_threshold, 0.95);
    }

    #[test]
    fn test_config_validation_valid() {
        let mut config = AppConfig::default();
        config.llm.api_key = "test-key".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut config = AppConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_threshold() {
        let mut config = AppConfig::default();
        config.cache.similarity_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_missing_api_key() {
        let config = AppConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_redaction_hides_api_key() {
        let mut config = AppConfig::default();
        config.llm.api_key = "secret-key-123".to_string();
        let redacted = config.redacted();
        assert!(redacted.contains("***REDACTED***"));
        assert!(!redacted.contains("secret-key-123"));
    }

    #[test]
    fn test_environment_detection() {
        let env = AppConfig::environment();
        assert!(env == "development" || env == "production");
    }
}
