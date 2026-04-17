//! Model Context Protocol (MCP) server implementation
//!
//! Provides comprehensive JSON-RPC 2.0 compliant MCP server for IDE integration.
//! Implements all core MCP methods: initialize, list_resources, call_tool, list_prompts, complete.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

use crate::{Error, Result};

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP error codes
pub mod mcp_error_codes {
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const PARSE_ERROR: i32 = -32700;
}

/// MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// MCP prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPPrompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<Value>,
}

/// MCP server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerCapabilities {
    pub tools: ToolCapabilities,
    pub resources: ResourceCapabilities,
    pub prompts: PromptCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapabilities {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCapabilities {
    pub subscribe: bool,
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCapabilities {
    pub list_changed: bool,
}

/// MCP server initialization info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPInitializationInfo {
    pub protocol_version: String,
    pub capabilities: MCPServerCapabilities,
    pub server_info: MCPServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP request (legacy compatibility)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MCPRequest {
    pub method: String,
    pub params: serde_json::Value,
}

/// MCP response (legacy compatibility)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MCPResponse {
    pub result: serde_json::Value,
}

/// Model Context Protocol (MCP) server implementation
pub struct MCPServer {
    tools: Arc<RwLock<HashMap<String, MCPTool>>>,
    resources: Arc<RwLock<HashMap<String, MCPResource>>>,
    prompts: Arc<RwLock<HashMap<String, MCPPrompt>>>,
    initialization_info: MCPInitializationInfo,
}

impl Default for MCPServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MCPServer {
    /// Create a new MCP server
    pub fn new() -> Self {
        info!("Creating MCP server");

        let initialization_info = MCPInitializationInfo {
            protocol_version: "2024-11-05".to_string(),
            capabilities: MCPServerCapabilities {
                tools: ToolCapabilities {
                    list_changed: false,
                },
                resources: ResourceCapabilities {
                    subscribe: false,
                    list_changed: false,
                },
                prompts: PromptCapabilities {
                    list_changed: false,
                },
            },
            server_info: MCPServerInfo {
                name: "Aegis-MCP".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let mut server = Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
            prompts: Arc::new(RwLock::new(HashMap::new())),
            initialization_info,
        };

        // Register default tools
        if let Err(e) = server.register_default_tools() {
            error!("Failed to register default tools: {}", e);
        }

        server
    }

    /// Register default MCP tools
    fn register_default_tools(&mut self) -> Result<()> {
        let cache_lookup_tool = MCPTool {
            name: "cache_lookup".to_string(),
            description: "Look up a cached response by prompt hash or semantic similarity".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The prompt text to look up"
                    },
                    "similarity_threshold": {
                        "type": "number",
                        "description": "Minimum similarity score (0-1)",
                        "default": 0.95
                    }
                },
                "required": ["prompt"]
            }),
        };

        let cache_stats_tool = MCPTool {
            name: "cache_stats".to_string(),
            description: "Get cache performance statistics".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        };

        let health_check_tool = MCPTool {
            name: "health_check".to_string(),
            description: "Check the health status of the Aegis-MCP server".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        };

        let mut tools = self.tools.try_write()
            .map_err(|e| Error::Lock(format!("Failed to acquire tools lock: {}", e)))?;

        tools.insert(cache_lookup_tool.name.clone(), cache_lookup_tool);
        tools.insert(cache_stats_tool.name.clone(), cache_stats_tool);
        tools.insert(health_check_tool.name.clone(), health_check_tool);

        info!("Registered default tools");
        Ok(())
    }

    /// Register a custom tool
    #[instrument(skip(self, tool))]
    pub async fn register_tool(&self, tool: MCPTool) -> Result<()> {
        let tool_name = tool.name.clone();
        let mut tools = self.tools.write().await;
        tools.insert(tool_name.clone(), tool);
        info!("Registered tool: {}", tool_name);
        Ok(())
    }

    /// Register a resource
    #[instrument(skip(self, resource))]
    pub async fn register_resource(&self, resource: MCPResource) -> Result<()> {
        let resource_uri = resource.uri.clone();
        let mut resources = self.resources.write().await;
        resources.insert(resource_uri.clone(), resource);
        info!("Registered resource: {}", resource_uri);
        Ok(())
    }

    /// Register a prompt
    #[instrument(skip(self, prompt))]
    pub async fn register_prompt(&self, prompt: MCPPrompt) -> Result<()> {
        let prompt_name = prompt.name.clone();
        let mut prompts = self.prompts.write().await;
        prompts.insert(prompt_name.clone(), prompt);
        info!("Registered prompt: {}", prompt_name);
        Ok(())
    }

    /// Handle JSON-RPC request
    #[instrument(skip(self, request))]
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling MCP request: {}", request.method);

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: mcp_error_codes::INVALID_REQUEST,
                    message: "Only JSON-RPC 2.0 is supported".to_string(),
                    data: None,
                }),
            };
        }

        // Route to appropriate handler
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "list_tools" => self.handle_list_tools().await,
            "call_tool" => self.handle_call_tool(request.params).await,
            "list_resources" => self.handle_list_resources().await,
            "list_prompts" => self.handle_list_prompts().await,
            "complete" => self.handle_complete(request.params).await,
            "ping" => self.handle_ping().await,
            _ => Err(Error::MCP(format!("Method not found: {}", request.method))),
        };

        match result {
            Ok(result_value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(result_value),
                error: None,
            },
            Err(e) => {
                error!("Error handling MCP request: {}", e);
                let (code, message) = self.error_to_code(&e);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code,
                        message,
                        data: Some(json!(e.to_string())),
                    }),
                }
            }
        }
    }

    /// Handle initialize method
    async fn handle_initialize(&self, params: Option<Value>) -> Result<Value> {
        debug!("Handling initialize");

        // Parse client capabilities if provided
        if let Some(params) = params {
            if let Some(client_info) = params.get("clientInfo") {
                info!(
                    "Client connected: {} {}",
                    client_info["name"].as_str().unwrap_or("unknown"),
                    client_info["version"].as_str().unwrap_or("unknown")
                );
            }
        }

        Ok(json!(self.initialization_info))
    }

    /// Handle list_tools method
    async fn handle_list_tools(&self) -> Result<Value> {
        debug!("Handling list_tools");

        let tools = self.tools.read().await;
        let tools_vec: Vec<&MCPTool> = tools.values().collect();

        Ok(json!({
            "tools": tools_vec
        }))
    }

    /// Handle call_tool method
    async fn handle_call_tool(&self, params: Option<Value>) -> Result<Value> {
        debug!("Handling call_tool");

        let params = params.ok_or_else(|| Error::MCP("Missing params".to_string()))?;
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::MCP("Missing tool name".to_string()))?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        info!("Calling tool: {} with args: {}", tool_name, arguments);

        // Handle built-in tools
        match tool_name {
            "health_check" => self.handle_health_check().await,
            "cache_stats" => self.handle_cache_stats().await,
            "cache_lookup" => self.handle_cache_lookup(arguments).await,
            _ => Err(Error::MCP(format!("Unknown tool: {}", tool_name))),
        }
    }

    /// Handle list_resources method
    async fn handle_list_resources(&self) -> Result<Value> {
        debug!("Handling list_resources");

        let resources = self.resources.read().await;
        let resources_vec: Vec<&MCPResource> = resources.values().collect();

        Ok(json!({
            "resources": resources_vec
        }))
    }

    /// Handle list_prompts method
    async fn handle_list_prompts(&self) -> Result<Value> {
        debug!("Handling list_prompts");

        let prompts = self.prompts.read().await;
        let prompts_vec: Vec<&MCPPrompt> = prompts.values().collect();

        Ok(json!({
            "prompts": prompts_vec
        }))
    }

    /// Handle complete method (autocomplete)
    async fn handle_complete(&self, params: Option<Value>) -> Result<Value> {
        debug!("Handling complete");

        let params = params.ok_or_else(|| Error::MCP("Missing params".to_string()))?;
        let _request_type = params.get("request_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::MCP("Missing request_type".to_string()))?;

        // Return empty completion for now
        Ok(json!({
            "completion": {
                "values": [],
                "total": 0,
                "has_more": false
            }
        }))
    }

    /// Handle ping method
    async fn handle_ping(&self) -> Result<Value> {
        Ok(json!({
            "status": "ok",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Handle health check tool
    async fn handle_health_check(&self) -> Result<Value> {
        Ok(json!({
            "status": "healthy",
            "server": "Aegis-MCP",
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Handle cache stats tool
    async fn handle_cache_stats(&self) -> Result<Value> {
        // Placeholder - will be integrated with actual cache engine
        Ok(json!({
            "cache_hits": 0,
            "cache_misses": 0,
            "total_requests": 0,
            "hit_rate": 0.0
        }))
    }

    /// Handle cache lookup tool
    async fn handle_cache_lookup(&self, arguments: Value) -> Result<Value> {
        let prompt = arguments.get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::MCP("Missing prompt".to_string()))?;

        // Placeholder - will be integrated with actual cache engine
        Ok(json!({
            "found": false,
            "prompt": prompt,
            "message": "Cache lookup not yet integrated with cache engine"
        }))
    }

    /// Convert error to JSON-RPC error code
    fn error_to_code(&self, error: &Error) -> (i32, String) {
        match error {
            Error::MCP(msg) if msg.contains("not found") => {
                (mcp_error_codes::METHOD_NOT_FOUND, msg.clone())
            }
            Error::MCP(msg) if msg.contains("params") => {
                (mcp_error_codes::INVALID_PARAMS, msg.clone())
            }
            Error::MCP(msg) => {
                (mcp_error_codes::INTERNAL_ERROR, msg.clone())
            }
            _ => (
                mcp_error_codes::INTERNAL_ERROR,
                "Internal server error".to_string(),
            ),
        }
    }

    /// Parse JSON-RPC request from JSON string
    pub fn parse_request(json_str: &str) -> Result<JsonRpcRequest> {
        serde_json::from_str(json_str)
            .map_err(|e| Error::Serialization(format!("Failed to parse JSON-RPC request: {}", e)))
    }

    /// Serialize JSON-RPC response to JSON string
    pub fn serialize_response(response: &JsonRpcResponse) -> Result<String> {
        serde_json::to_string(response)
            .map_err(|e| Error::Serialization(format!("Failed to serialize JSON-RPC response: {}", e)))
    }
}

/// MCP server builder for convenient configuration
pub struct MCPServerBuilder {
    server: MCPServer,
}

impl MCPServerBuilder {
    pub fn new() -> Self {
        Self {
            server: MCPServer::new(),
        }
    }

    pub fn with_tool(mut self, tool: MCPTool) -> Self {
        let tool_name = tool.name.clone();
        {
            let mut tools = self.server.tools.try_write().unwrap();
            tools.insert(tool_name.clone(), tool);
        }
        self
    }

    pub fn with_resource(mut self, resource: MCPResource) -> Self {
        let resource_uri = resource.uri.clone();
        {
            let mut resources = self.server.resources.try_write().unwrap();
            resources.insert(resource_uri.clone(), resource);
        }
        self
    }

    pub fn with_prompt(mut self, prompt: MCPPrompt) -> Self {
        let prompt_name = prompt.name.clone();
        {
            let mut prompts = self.server.prompts.try_write().unwrap();
            prompts.insert(prompt_name.clone(), prompt);
        }
        self
    }

    pub fn build(self) -> MCPServer {
        self.server
    }
}

impl Default for MCPServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let server = MCPServer::new();
        assert_eq!(server.initialization_info.server_info.name, "Aegis-MCP");
    }

    #[test]
    fn test_json_rpc_request_parsing() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":null}"#;
        let request = MCPServer::parse_request(json_str).unwrap();
        assert_eq!(request.method, "ping");
        assert_eq!(request.jsonrpc, "2.0");
    }

    #[test]
    fn test_json_rpc_response_serialization() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: Some(json!({"status": "ok"})),
            error: None,
        };

        let json_str = MCPServer::serialize_response(&response).unwrap();
        assert!(json_str.contains("jsonrpc"));
        assert!(json_str.contains("result"));
    }

    #[test]
    fn test_mcp_tool_creation() {
        let tool = MCPTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
    }

    #[test]
    fn test_error_code_conversion() {
        let server = MCPServer::new();
        let mcp_error = Error::MCP("Method not found: test".to_string());
        let (code, message) = server.error_to_code(&mcp_error);

        assert_eq!(code, mcp_error_codes::METHOD_NOT_FOUND);
        assert!(message.contains("not found"));
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = MCPServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "ping".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let server = MCPServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "initialize".to_string(),
            params: Some(json!({"clientInfo": {"name": "test-client", "version": "1.0"}})),
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        assert!(result.get("protocol_version").is_some());
        assert!(result.get("capabilities").is_some());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let server = MCPServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "list_tools".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();
        assert!(!tools.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_json_rpc_version() {
        let server = MCPServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: json!(1),
            method: "ping".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, mcp_error_codes::INVALID_REQUEST);
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let server = MCPServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "unknown_method".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, mcp_error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_register_custom_tool() {
        let server = MCPServer::new();
        let custom_tool = MCPTool {
            name: "custom_tool".to_string(),
            description: "A custom tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let result = server.register_tool(custom_tool).await;
        assert!(result.is_ok());

        // Verify tool is registered
        let tools = server.tools.read().await;
        assert!(tools.contains_key("custom_tool"));
    }
}