//! Model Context Protocol (MCP) server implementation
//!
//! Placeholder module for MCP functionality - will be implemented in Issue #9.

use serde::{Deserialize, Serialize};

/// MCP request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MCPRequest {
    pub method: String,
    pub params: serde_json::Value,
}

/// MCP response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MCPResponse {
    pub result: serde_json::Value,
}

/// MCP server - will be fully implemented in Issue #9
pub struct MCPServer;

impl Default for MCPServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MCPServer {
    pub fn new() -> Self {
        Self
    }
}
