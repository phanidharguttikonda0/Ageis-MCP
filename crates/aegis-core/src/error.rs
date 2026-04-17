//! Error types for Aegis-MCP
//!
//! Centralized error handling using thiserror and anyhow.

use thiserror::Error;

/// Main error type for Aegis-MCP
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Redis error: {0}")]
    Redis(String),
    
    #[error("Embedding generation error: {0}")]
    Embedding(String),
    
    #[error("LLM API error: {0}")]
    LLM(String),
    
    #[error("MCP protocol error: {0}")]
    MCP(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type alias using our Error
pub type Result<T> = std::result::Result<T, Error>;
