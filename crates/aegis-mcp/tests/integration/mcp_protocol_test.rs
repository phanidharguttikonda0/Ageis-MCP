//! MCP Protocol Compliance Integration Tests
//!
//! Tests the complete MCP JSON-RPC 2.0 protocol implementation including
//! all methods, error handling, and tool invocation.

use aegis_core::mcp::{
    JsonRpcRequest, JsonRpcResponse, MCPServer, MCPTool,
    mcp_error_codes, MCPServerBuilder
};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};

/// Helper function to create a test JSON-RPC request
fn create_test_request(id: Value, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.to_string(),
        params,
    }
}

/// Helper function to execute a request and get the response
async fn execute_request(server: &MCPServer, request: JsonRpcRequest) -> JsonRpcResponse {
    server.handle_request(request).await
}

#[tokio::test]
async fn test_mcp_initialization_handshake() {
    let server = MCPServer::new();

    // Test proper initialization request
    let request = create_test_request(
        json!(1),
        "initialize",
        Some(json!({
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    assert!(result.get("protocol_version").is_some());
    assert!(result.get("capabilities").is_some());
    assert!(result.get("server_info").is_some());

    // Verify server info
    let server_info = result.get("server_info").unwrap();
    assert_eq!(server_info.get("name").unwrap().as_str().unwrap(), "Aegis-MCP");
}

#[tokio::test]
async fn test_mcp_tool_discovery() {
    let server = MCPServer::new();

    // Test list_tools method
    let request = create_test_request(json!(1), "list_tools", None);
    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

    // Verify expected tools exist
    let tool_names: Vec<&str> = tools.iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(tool_names.contains(&"health_check"));
    assert!(tool_names.contains(&"cache_stats"));
    assert!(tool_names.contains(&"cache_lookup"));
    assert!(tool_names.contains(&"invalidate_cache"));
    assert!(tool_names.contains(&"warm_cache"));
    assert!(tool_names.contains(&"set_similarity_threshold"));
}

#[tokio::test]
async fn test_mcp_tool_invocation() {
    let server = MCPServer::new();

    // Test health_check tool
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "health_check",
            "arguments": {}
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    assert_eq!(result.get("status").unwrap().as_str().unwrap(), "healthy");
    assert_eq!(result.get("server").unwrap().as_str().unwrap(), "Aegis-MCP");
}

#[tokio::test]
async fn test_mcp_cache_lookup_tool() {
    let server = MCPServer::new();

    // Test cache_lookup tool
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "cache_lookup",
            "arguments": {
                "prompt": "test prompt for cache lookup",
                "similarity_threshold": 0.95
            }
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    assert!(result.get("prompt").is_some());
    assert!(result.get("similarity_threshold").is_some());
}

#[tokio::test]
async fn test_mcp_invalid_params_error() {
    let server = MCPServer::new();

    // Test missing required parameters
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "cache_lookup",
            "arguments": {} // Missing required "prompt" parameter
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, mcp_error_codes::INVALID_PARAMS);
    assert!(error.message.contains("Missing prompt"));
}

#[tokio::test]
async fn test_mcp_method_not_found() {
    let server = MCPServer::new();

    // Test unknown method
    let request = create_test_request(
        json!(1),
        "unknown_method",
        None
    );

    let response = execute_request(&server, request).await;

    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, mcp_error_codes::METHOD_NOT_FOUND);
}

#[tokio::test]
async fn test_mcp_invalid_json_rpc_version() {
    let server = MCPServer::new();

    // Test invalid JSON-RPC version
    let request = JsonRpcRequest {
        jsonrpc: "1.0".to_string(), // Wrong version
        id: json!(1),
        method: "ping".to_string(),
        params: None,
    };

    let response = execute_request(&server, request).await;

    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, mcp_error_codes::INVALID_REQUEST);
    assert!(error.message.contains("JSON-RPC 2.0"));
}

#[tokio::test]
async fn test_mcp_ping() {
    let server = MCPServer::new();

    let request = create_test_request(json!(1), "ping", None);
    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    assert_eq!(result.get("status").unwrap().as_str().unwrap(), "ok");
    assert!(result.get("timestamp").is_some());
}

#[tokio::test]
async fn test_mcp_request_timeout() {
    let server = MCPServer::new();

    // Test that requests complete within reasonable time
    let request = create_test_request(json!(1), "list_tools", None);

    let result = timeout(Duration::from_secs(1), execute_request(&server, request)).await;

    assert!(result.is_ok(), "Request should complete within timeout");
}

#[tokio::test]
async fn test_mcp_custom_tool_registration() {
    let server = MCPServer::new();

    // Register a custom tool
    let custom_tool = MCPTool {
        name: "custom_tool".to_string(),
        description: "A custom test tool".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "value": {"type": "string"}
            },
            "required": ["value"]
        }),
    };

    server.register_tool(custom_tool).await.unwrap();

    // Verify the tool is available
    let request = create_test_request(json!(1), "list_tools", None);
    let response = execute_request(&server, request).await;

    let result = response.result.unwrap();
    let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

    let tool_names: Vec<&str> = tools.iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(tool_names.contains(&"custom_tool"));
}

#[tokio::test]
async fn test_mcp_cache_stats_tool() {
    let server = MCPServer::new();

    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "cache_stats",
            "arguments": {}
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let result = response.result.unwrap();
    // Verify response structure (even though values are placeholders)
    assert!(result.get("cache_hits").is_some());
    assert!(result.get("cache_misses").is_some());
    assert!(result.get("total_requests").is_some());
}

#[tokio::test]
async fn test_mcp_invalidate_cache_tool() {
    let server = MCPServer::new();

    // Test invalidation without confirmation (should fail)
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "invalidate_cache",
            "arguments": {}
        }))
    );

    let response = execute_request(&server, request).await;

    // Should succeed but indicate cache engine not available
    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_mcp_warm_cache_tool() {
    let server = MCPServer::new();

    // Test cache warming with prompts
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "warm_cache",
            "arguments": {
                "prompts": ["test prompt 1", "test prompt 2"]
            }
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result.get("success").unwrap().as_bool().unwrap(), true);
    assert_eq!(result.get("prompts_count").unwrap().as_u64().unwrap(), 2);
}

#[tokio::test]
async fn test_mcp_set_similarity_threshold_tool() {
    let server = MCPServer::new();

    // Test setting similarity threshold
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "set_similarity_threshold",
            "arguments": {
                "threshold": 0.98
            }
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result.get("success").unwrap().as_bool().unwrap(), true);
    assert_eq!(result.get("new_threshold").unwrap().as_f64().unwrap(), 0.98);
}

#[tokio::test]
async fn test_mcp_invalid_threshold_value() {
    let server = MCPServer::new();

    // Test invalid threshold value (> 1.0)
    let request = create_test_request(
        json!(1),
        "call_tool",
        Some(json!({
            "name": "set_similarity_threshold",
            "arguments": {
                "threshold": 1.5 // Invalid: > 1.0
            }
        }))
    );

    let response = execute_request(&server, request).await;

    assert!(response.error.is_some());
    assert!(response.error.as_ref().unwrap().message.contains("between 0.0 and 1.0"));
}

#[tokio::test]
async fn test_mcp_concurrent_requests() {
    let server = std::sync::Arc::new(server);

    // Test handling multiple concurrent requests
    let mut handles = Vec::new();

    for i in 0..10 {
        let server_clone = server.clone();
        let handle = tokio::spawn(async move {
            let request = create_test_request(
                json!(i),
                "ping",
                None
            );
            execute_request(&server_clone, request).await
        });
        handles.push(handle);
    }

    // All requests should complete successfully
    for handle in handles {
        let response = handle.await.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }
}

#[tokio::test]
async fn test_mcp_response_id_correlation() {
    let server = MCPServer::new();

    // Test that response IDs match request IDs
    let test_ids = vec![json!(1), json!("test-id"), json!(null), json!(42)];

    for id in test_ids {
        let request = create_test_request(id.clone(), "ping", None);
        let response = execute_request(&server, request).await;

        assert_eq!(response.id, id, "Response ID should match request ID");
    }
}