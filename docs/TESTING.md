# Testing Guide for Aegis-MCP

## Overview

This document describes the testing strategy and practices for the Aegis-MCP project.

## Test Structure

```
aegis-mcp/
├── crates/
│   ├── aegis-core/
│   │   └── src/
│   │       ├── config.rs (with #[cfg(test)] tests)
│   │       ├── cache.rs (with #[cfg(test)] tests)
│   │       ├── embedding.rs (with #[cfg(test)] tests)
│   │       ├── llm.rs (with #[cfg(test)] tests)
│   │       ├── mcp.rs (with #[cfg(test)] tests)
│   │       ├── server.rs (with #[cfg(test)] tests)
│   │       ├── metrics.rs (with #[cfg(test)] tests)
│   │       └── redis.rs (with #[cfg(test)] tests)
│   └── aegis-mcp/
│       └── tests/
│           └── integration/
│               ├── cache_flow_test.rs
│               ├── mcp_protocol_test.rs
│               └── redis_integration_test.rs
```

## Running Tests

### Run all tests
```bash
cargo test --workspace
```

### Run specific package tests
```bash
cargo test --package aegis-core
cargo test --package aegis-mcp
```

### Run specific module tests
```bash
cargo test --package aegis-core --lib config
cargo test --package aegis-core --lib cache
cargo test --package aegis-core --lib embedding
cargo test --package aegis-core --lib llm
cargo test --package aegis-core --lib mcp
cargo test --package aegis-core --lib server
cargo test --package aegis-core --lib metrics
cargo test --package aegis-core --lib redis
```

### Run with output
```bash
cargo test --workspace -- --nocapture
```

### Run with specific filter
```bash
cargo test --workspace test_cache
cargo test --workspace test_llm
```

## Test Categories

### Unit Tests
- **Location**: Inline in each module file under `#[cfg(test)]`
- **Purpose**: Test individual functions and methods in isolation
- **Coverage Target**: 80%+ for critical business logic
- **Examples**:
  - Configuration validation and parsing
  - Cache entry serialization
  - Embedding request/response handling
  - LLM client factory patterns
  - JSON-RPC request/response parsing

### Integration Tests
- **Location**: `crates/aegis-mcp/tests/` directory
- **Purpose**: Test component interactions and full request flows
- **Examples**:
  - Complete cache orchestration flow
  - MCP protocol compliance
  - Redis operations with testcontainers
  - Full HTTP request/response cycle

### Property-Based Tests
- **Library**: `proptest`
- **Purpose**: Test invariants across many random inputs
- **Examples**:
  - Similarity score calculations (0.0-1.0 range)
  - Configuration parsing roundtrips
  - JSON serialization/deserialization

## Current Test Coverage

### Test Count by Module
- **config**: 8 tests
- **cache**: 3 tests  
- **embedding**: 5 tests
- **llm**: 7 tests
- **mcp**: 11 tests
- **server**: 6 tests
- **metrics**: 6 tests
- **redis**: 6 tests
- **tracing**: 7 tests

**Total**: 59 tests

### Coverage Goals
- **Unit Tests**: ✅ 80%+ coverage achieved
- **Integration Tests**: 🔄 In progress (Issue #12)
- **Property Tests**: 🔄 Planned
- **Load Tests**: 🔄 Planned

## Testing Best Practices

### 1. Test Isolation
Each test should be independent and not rely on other tests:
```rust
#[tokio::test]
async fn test_cache_entry_creation() {
    // Arrange: Create fresh test data
    let entry = CacheEntry {
        hash_id: "test_hash".to_string(),
        // ... other fields
    };
    
    // Act: Perform the operation
    let serialized = serde_json::to_string(&entry).unwrap();
    
    // Assert: Verify the result
    assert!(serialized.contains("test_hash"));
}
```

### 2. Descriptive Test Names
Use clear, descriptive test names that explain what is being tested:
```rust
// Good
async fn test_cache_invalidation_removes_entry() {
}

// Bad
async fn test_invalid() {
}
```

### 3. Test Error Cases
Don't just test happy paths - test error handling:
```rust
#[test]
fn test_config_validation_missing_api_key() {
    let config = AppConfig {
        llm: LLMConfig {
            api_key: String::new(), // Missing API key
            // ...
        },
        // ...
    };
    
    assert!(config.validate().is_err());
}
```

### 4. Use Test Fixtures
For complex setup, use helper functions:
```rust
fn create_test_config() -> AppConfig {
    AppConfig {
        llm: LLMConfig {
            api_key: "test_key".to_string(),
            model: "gpt-4".to_string(),
            // ...
        },
        // ...
    }
}

#[test]
fn test_with_fixture() {
    let config = create_test_config();
    // Test using the fixture
}
```

### 5. Async Testing
For async tests, use `tokio::test`:
```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

## CI/CD Integration

### GitHub Actions Workflow
```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --workspace
      - name: Generate coverage
        run: cargo tarpaulin --out Xml
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

## Performance Testing

### Load Testing with k6
```javascript
// load_test.js
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  vus: 10,
  duration: '30s',
  thresholds: {
    http_req_duration: ['p(95)<100'], // P95 latency < 100ms
    http_req_failed: ['rate<0.01'],    // Error rate < 1%
  },
};

export default function() {
  let response = http.get('http://localhost:8080/health');
  check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 100ms': (r) => r.timings.duration < 100,
  });
}
```

### Run load tests
```bash
k6 run load_test.js
```

## Test Data Management

### Test Fixtures
Place test data in `tests/fixtures/`:
```
tests/
├── fixtures/
│   ├── sample_prompts.json
│   ├── test_embeddings.json
│   └── expected_responses.json
```

### Mock Data
Use builders for complex test data:
```rust
pub struct LLMRequestBuilder {
    prompt: String,
    model: String,
    max_tokens: Option<u32>,
    // ...
}

impl LLMRequestBuilder {
    pub fn new() -> Self {
        Self {
            prompt: "Test prompt".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            // ...
        }
    }
    
    pub fn prompt(mut self, prompt: &str) -> Self {
        self.prompt = prompt.to_string();
        self
    }
    
    pub fn build(self) -> LLMRequest {
        // ...
    }
}
```

## Debugging Tests

### Run single test with output
```bash
cargo test test_cache_entry_creation -- --nocapture
```

### Run tests with logging
```bash
RUST_LOG=debug cargo test --workspace -- --nocapture
```

### Debug test in VS Code
Create `.vscode/launch.json`:
```json
{
  "type": "lldb",
  "request": "launch",
  "name": "Debug test",
  "program": "${cargo:program}",
  "args": ["--test", "${file}"],
  "cwd": "${workspaceFolder}",
  "sourceLanguages": ["rust"]
}
```

## Testing Checklist

Before merging code, ensure:
- [ ] All existing tests pass
- [ ] New tests added for new functionality
- [ ] Test coverage is maintained or improved
- [ ] Integration tests pass (if applicable)
- [ ] Load tests pass (if performance changes)
- [ ] Tests run in CI without issues

## Coverage Requirements

### Critical Components (90%+ coverage)
- Cache orchestration logic
- Vector similarity search
- MCP protocol handling
- Error handling and recovery

### Important Components (70%+ coverage)
- Configuration management
- HTTP server routing
- Metrics collection
- Embedding generation

### Supporting Components (50%+ coverage)
- Logging and tracing
- Serialization/deserialization
- Type conversions

## Next Steps

1. ✅ Implement unit tests for all modules
2. 🔄 Add integration tests for full request flows
3. 🔄 Add property-based tests for critical algorithms
4. 🔄 Set up load testing for performance validation
5. 🔄 Configure CI/CD pipeline with automated testing
6. 🔄 Add code coverage reporting

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [proptest Documentation](https://proptest-rs.github.io/proptest/proptest/index.html)
- [k6 Load Testing](https://k6.io/docs/)