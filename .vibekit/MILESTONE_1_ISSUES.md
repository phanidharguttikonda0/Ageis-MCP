# Milestone 1: Foundation & Core Infrastructure

**Goal:** Establish the foundational infrastructure for Aegis-MCP including project scaffolding, Redis integration, basic HTTP server, and vector similarity search capability.

**Target:** 2-3 weeks

**Success Criteria:**
- Axum web server running with health check endpoint
- Redis connection pool with vector search capability
- Basic prompt caching flow (embedding → search → return/forward)
- Comprehensive logging and observability
- Integration tests for core components

---

## Phase 1: Project Foundation (Week 1)

### Issue #1: Cargo Workspace & Dependency Setup
**Priority:** P0 - Critical
**Labels:** `arch`, `foundational`

**Description:**
Set up the Rust project structure with proper Cargo workspace organization for a production-ready MCP server.

**Technical Requirements:**
- Configure Cargo.toml with appropriate dependencies:
  - `axum = "0.7"` (web framework)
  - `tokio = { version = "1.35", features = ["full"] }` (async runtime)
  - `redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }` (Redis client)
  - `serde = { version = "1.0", features = ["derive"] }` (serialization)
  - `tracing = "0.1"` and `tracing-subscriber = "0.3"` (observability)
  - `anyhow = "1.0"` (error handling)
  - `config = "0.14"` (configuration management)
  - `uuid = { version = "1.6", features = ["v4", "serde"] }` (request IDs)
- Set up workspace structure: `crates/` directory with modular crates
- Create binary crate: `crates/aegis-mcp/`
- Create library crate: `crates/aegis-core/` (shared business logic)
- Configure release profile optimizations
- Set up feature flags for development/testing/production

**Acceptance Criteria:**
- [ ] `cargo build` succeeds with all dependencies
- [ ] Workspace has 2 crates: `aegis-mcp` (binary) and `aegis-core` (library)
- [ ] All dependencies are latest stable versions
- [ ] Feature flags work correctly
- [ ] README updated with build instructions

**Complexity:** Low
**Estimated Time:** 2-3 hours

---

### Issue #2: Configuration Management System
**Priority:** P0 - Critical
**Labels:** `arch`, `foundational`

**Description:**
Implement a robust configuration management system using the `config` crate with support for multiple sources (environment variables, config files, defaults).

**Technical Requirements:**
- Configuration sources (priority order):
  1. Environment variables
  2. `config/production.toml` or `config/development.toml`
  3. Default values in code
- Configuration structure:
  ```toml
  [server]
  host = "127.0.0.1"
  port = 8080
  max_connections = 1000

  [redis]
  url = "redis://localhost:6379"
  pool_size = 10
  connection_timeout = 5000
  max_lifetime = 3600

  [cache]
  similarity_threshold = 0.95
  max_cache_size = 10000
  ttl_seconds = 86400

  [llm]
  upstream_url = "https://api.openai.com/v1"
  api_key = "${OPENAI_API_KEY}"
  model = "gpt-4"
  timeout = 30000
  max_retries = 3

  [observability]
  log_level = "info"
  jaeger_endpoint = "http://localhost:4318"
  ```
- Validation on startup (fail fast on invalid config)
- Secret redaction in logs
- Hot-reload support for non-critical settings

**Acceptance Criteria:**
- [ ] Configuration loads from multiple sources correctly
- [ ] Invalid configuration causes startup failure with clear error messages
- [ ] Secrets are redacted from logs
- [ ] Integration tests for all config sources
- [ ] Documentation in `docs/CONFIGURATION.md`

**Complexity:** Medium
**Estimated Time:** 4-6 hours

---

### Issue #3: Structured Logging & Tracing Setup
**Priority:** P0 - Critical
**Labels:** `observability`, `foundational`

**Description:**
Implement comprehensive structured logging and distributed tracing using the `tracing` ecosystem for deep observability into the request pipeline.

**Technical Requirements:**
- `tracing-subscriber` configuration with:
  - JSON formatting for production
  - Pretty printing for development
  - Log level filtering via environment variable
- Span creation for:
  - Entire request lifecycle (`request_id` as span ID)
  - Embedding generation (`embedding_generation`)
  - Redis queries (`redis_query`, `redis_search`)
  - Upstream LLM calls (`upstream_llm_request`)
  - Cache operations (`cache_hit`, `cache_miss`)
- Structured fields:
  - `request_id` (UUID)
  - `client_id` (when available)
  - `prompt_length` (character count)
  - `similarity_score` (for cache hits)
  - `cache_hit` (boolean)
  - `latency_ms` (per-stage timing)
  - `error` (when applicable)
- OpenTelemetry integration:
  - Jaeger exporter for distributed tracing
  - Prometheus metrics exporter
- Log sampling: 100% for errors, 10% for success in high-traffic scenarios

**Acceptance Criteria:**
- [ ] All request stages are traced with proper parent-child relationships
- [ ] Logs are structured JSON with all required fields
- [ ] Traces visible in Jaeger UI
- [ ] Metrics exported to Prometheus endpoint
- [ ] Log levels configurable via environment variable
- [ ] No sensitive data (API keys, prompts) in logs

**Complexity:** Medium
**Estimated Time:** 6-8 hours

---

## Phase 2: Core Infrastructure (Week 1-2)

### Issue #4: Redis Connection Pool & Vector Search
**Priority:** P0 - Critical
**Labels:** `arch`, `datastore`, `performance`

**Description:**
Implement a robust Redis connection pool with vector similarity search capabilities using RediSearch.

**Technical Requirements:**
- Connection pooling:
  - Use `deadpool-redis` or `bb8` for pool management
  - Configurable pool size (default: 10 connections)
  - Connection timeout and retry logic
  - Health check on connection checkout
- Vector search module (`aegis-core/src/cache/redis.rs`):
  ```rust
  pub struct VectorSearchRequest {
      pub embedding: Vec<f32>,
      pub threshold: f32,
      pub limit: usize,
  }

  pub struct VectorSearchResult {
      pub hash_id: String,
      pub similarity_score: f32,
      pub response_payload: String,
  }

  pub async fn search_similar(
      pool: &RedisPool,
      request: VectorSearchRequest,
  ) -> Result<Vec<VectorSearchResult>, CacheError>
  ```
- RediSearch index creation:
  ```rust
  async fn create_vector_index(
      conn: &mut Connection,
      dimension: usize,
  ) -> Result<(), CacheError>
  ```
- Cache storage operations:
  - `store_cache_entry(prompt, embedding, response)`
  - `get_cache_entry(hash_id)`
  - `invalidate_cache(hash_id)`
- Error handling for Redis failures (circuit breaker pattern)

**Acceptance Criteria:**
- [ ] Connection pool creates and maintains connections
- [ ] Vector search returns results ranked by similarity (highest first)
- [ ] Similarity scores are computed correctly (cosine distance)
- [ ] Index creation is idempotent (safe to run multiple times)
- [ ] Connection failures trigger circuit breaker
- [ ] Integration tests with real Redis instance
- [ ] Performance: < 5ms for vector search with 10K entries

**Complexity:** High
**Estimated Time:** 12-16 hours

---

### Issue #5: HTTP Server & Basic Routing
**Priority:** P0 - Critical
**Labels:** `arch`, `web`

**Description:**
Implement the Axum HTTP server with routing for health checks, metrics, and future MCP endpoints.

**Technical Requirements:**
- Server configuration:
  ```rust
  pub struct ServerConfig {
      pub host: String,
      pub port: u16,
      pub max_connections: usize,
  }
  ```
- Routes:
  ```
  GET  /health                    - Health check endpoint
  GET  /metrics                   - Prometheus metrics
  GET  /stats/cache               - Cache statistics
  POST /api/v1/cache/invalidate   - Cache invalidation
  POST /api/v1/cache/warmup       - Cache warm-up (future)
  ```
- Request middleware:
  - Request ID generation (UUID)
  - Request logging (method, path, status, latency)
  - CORS configuration (configurable origins)
  - Timeout middleware (global timeout)
- Response compression: gzip for large payloads
- Graceful shutdown: handle SIGTERM/SIGINT, drain existing connections

**Acceptance Criteria:**
- [ ] Server starts and binds to configured host:port
- [ ] Health check returns `{"status":"ok"}`
- [ ] Metrics endpoint returns Prometheus format
- [ ] All requests have unique `request_id` in response headers
- [ ] Graceful shutdown completes within 30 seconds
- [ ] Integration tests for all endpoints
- [ ] Load test: handle 1000 concurrent connections

**Complexity:** Medium
**Estimated Time:** 8-10 hours

---

### Issue #6: Embedding Generation Service
**Priority:** P1 - High
**Labels:** `feature`, `ml`

**Description:**
Implement the embedding generation service that converts text prompts into vector representations for similarity search.

**Technical Requirements:**
- Support multiple embedding providers:
  - OpenAI `text-embedding-ada-002` (1536 dimensions)
  - Local model via `candle` or `rust-bert` (future)
- Embedding service interface:
  ```rust
  pub trait EmbeddingService: Send + Sync {
      async fn generate_embedding(
          &self,
          text: &str,
      ) -> Result<Vec<f32>, EmbeddingError>;

      fn model_info(&self) -> ModelInfo;
  }
  ```
- Error handling:
  - Rate limiting from upstream API
  - Retry with exponential backoff
  - Fallback to alternative provider if configured
- Caching: Cache embeddings locally (in-memory) for identical prompts
- Observability: Track embedding generation latency and API costs

**Acceptance Criteria:**
- [ ] OpenAI embedding provider implemented
- [ ] Embeddings are correct dimensionality (1536 for ada-002)
- [ ] Rate limiting and retries work correctly
- [ ] Error handling covers all failure modes
- [ ] Unit tests for embedding normalization
- [ ] Integration test with real OpenAI API (mocked in CI)

**Complexity:** Medium-High
**Estimated Time:** 10-12 hours

---

## Phase 3: Core Caching Logic (Week 2)

### Issue #7: Cache Orchestration Engine
**Priority:** P0 - Critical
**Labels:** `arch`, `core-logic`

**Description:**
Implement the core caching orchestration that coordinates embedding generation, vector search, cache lookup/miss handling, and upstream LLM forwarding.

**Technical Requirements:**
- Main orchestration flow:
  ```rust
  pub async fn handle_prompt_request(
      state: &AppState,
      request: PromptRequest,
  ) -> Result<PromptResponse, RequestError>
  ```
- Processing stages:
  1. Extract prompt text from request
  2. Generate embedding (or fetch from local cache)
  3. Query Redis for similar cached prompts
  4. If similarity > 0.95: return cached response
  5. Else: forward to upstream LLM
  6. Asynchronously write new entry to Redis
  7. Return response to client
- Async write-back pattern:
  ```rust
  // Fire-and-forget cache write
  tokio::spawn(async move {
      if let Err(e) = store_cache_entry(...).await {
          tracing::error!("Failed to cache: {:?}", e);
      }
  });
  ```
- Deduplication: Use in-memory `Arc<Mutex<HashSet>>` to deduplicate concurrent identical requests
- Timeout handling: Per-stage timeouts with cancellation

**Acceptance Criteria:**
- [ ] Cache hits return within 10ms (excluding network latency)
- [ ] Cache misses forward to upstream LLM correctly
- [ ] Async cache writes don't block response
- [ ] Concurrent identical requests are deduplicated
- [ ] Timeouts are enforced at each stage
- [ ] Comprehensive integration tests
- [ ] Performance: handle 100 requests/second

**Complexity:** High
**Estimated Time:** 16-20 hours

---

### Issue #8: Upstream LLM Client
**Priority:** P0 - Critical
**Labels:** `integration`, `external-api`

**Description:**
Implement the upstream LLM API client for forwarding cache misses to the actual LLM provider.

**Technical Requirements:**
- Support multiple LLM providers:
  - OpenAI (Chat Completions API)
  - Anthropic (Claude API)
  - Azure OpenAI
- Client interface:
  ```rust
  pub trait LLMClient: Send + Sync {
      async fn complete(
          &self,
          request: CompletionRequest,
      ) -> Result<CompletionResponse, LLMError>;
  }
  ```
- Request transformation:
  - Map MCP request format to provider-specific format
  - Handle streaming vs non-streaming responses
  - Inject system messages/configurations
- Response normalization:
  - Convert provider responses to standard format
  - Extract token usage for billing/analytics
- Reliability features:
  - Automatic retries with exponential backoff
  - Circuit breaker for failing endpoints
  - Timeout enforcement
  - Request/response logging (PII redacted)

**Acceptance Criteria:**
- [ ] OpenAI client implemented and tested
- [ ] Retry logic works correctly
- [ ] Circuit breaker opens after repeated failures
- [ ] Token usage is tracked and logged
- [ ] Streaming responses are supported
- [ ] Integration tests with mocked LLM APIs
- [ ] Error handling covers all failure modes

**Complexity:** High
**Estimated Time:** 14-18 hours

---

## Phase 4: MCP Protocol Implementation (Week 2-3)

### Issue #9: MCP JSON-RPC Server
**Priority:** P1 - High
**Labels:** `feature`, `protocol`

**Description:**
Implement the Model Context Protocol (MCP) JSON-RPC 2.0 server for IDE integration.

**Technical Requirements:**
- JSON-RPC 2.0 compliance:
  ```rust
  pub struct JsonRpcRequest {
      pub jsonrpc: "2.0",
      pub id: serde_json::Value,
      pub method: String,
      pub params: Option<serde_json::Value>,
  }

  pub struct JsonRpcResponse {
      pub jsonrpc: "2.0",
      pub id: serde_json::Value,
      pub result: Option<serde_json::Value>,
      pub error: Option<JsonRpcError>,
  }
  ```
- MCP method handlers:
  - `initialize` - Server initialization handshake
  - `list_resources` - Expose available resources (stats, config)
  - `call_tool` - Tool invocation (cache management operations)
  - `list_prompts` - Available prompt templates
  - `complete` - Main autocomplete completion endpoint
- Transport layer: HTTP POST with JSON content type
- Request validation and error handling per MCP spec
- Capability negotiation during initialization

**Acceptance Criteria:**
- [ ] All MCP methods implemented per spec
- [ ] JSON-RPC 2.0 compliant responses
- [ ] Error messages follow MCP error codes
- [ ] Initialize handshake works correctly
- [ ] Tools and resources are discoverable
- [ ] Integration tests with MCP test client

**Complexity:** High
**Estimated Time:** 16-20 hours

---

### Issue #10: MCP Tool Integration
**Priority:** P1 - High
**Labels:** `feature`, `ux`

**Description:**
Implement MCP tools for cache management and monitoring that can be invoked from the IDE.

**Technical Requirements:**
- Tool definitions:
  ```json
  {
      "name": "invalidate_cache",
      "description": "Invalidate cached entries by criteria",
      "inputSchema": {
          "type": "object",
          "properties": {
              "hash_id": {"type": "string"},
              "model_name": {"type": "string"},
              "older_than_seconds": {"type": "integer"}
          }
      }
  }
  ```
- Available tools:
  - `invalidate_cache` - Remove cached entries
  - `cache_stats` - Get cache statistics
  - `warm_cache` - Pre-warm cache with common prompts
  - `set_similarity_threshold` - Adjust cache threshold
- Tool implementation:
  - Input validation
  - Authorization checks (if configured)
  - Auditing of tool invocations
  - Structured responses

**Acceptance Criteria:**
- [ ] All tools are callable via MCP `call_tool`
- [ ] Input validation rejects invalid parameters
- [ ] Tool invocations are logged
- [ ] Error responses are informative
- [ ] Tools work from Cursor IDE integration
- [ ] Integration tests for all tools

**Complexity:** Medium
**Estimated Time:** 8-10 hours

---

## Phase 5: Testing & Documentation (Week 3)

### Issue #11: Comprehensive Testing Suite
**Priority:** P1 - High
**Labels:** `testing`, `quality`

**Description:**
Implement a comprehensive testing suite including unit tests, integration tests, and property-based tests.

**Technical Requirements:**
- Unit tests (target: 80%+ coverage):
  - Configuration loading and validation
  - Embedding generation (mocked)
  - Vector search logic
  - Cache orchestration
  - LLM client (mocked)
- Integration tests:
  - Redis operations (testcontainers for Redis)
  - Full request flow (prompt → cache → response)
  - MCP protocol compliance
  - Error scenarios (Redis failure, LLM failure)
- Property-based tests (using `proptest`):
  - Similarity score calculations
  - Configuration parsing
  - Request/response serialization
- Load tests (using `k6` or `locust`):
  - 100 requests/second sustained
  - Cache hit rate > 80% for similar prompts
  - P95 latency < 100ms for cache hits

**Acceptance Criteria:**
- [ ] Unit test coverage > 80%
- [ ] All integration tests pass
- [ ] Load test meets performance targets
- [ ] Tests run in CI pipeline
- [ ] Test documentation in `docs/TESTING.md`

**Complexity:** Medium-High
**Estimated Time:** 20-24 hours

---

### Issue #12: Production Deployment Guide
**Priority:** P2 - Medium
**Labels:** `documentation`, `operations`

**Description:**
Create comprehensive documentation for deploying and operating Aegis-MCP in production.

**Technical Requirements:**
- Deployment documentation:
  - Docker containerization (Dockerfile, docker-compose)
  - Kubernetes deployment manifests
  - Environment variable reference
  - Production configuration best practices
- Operations documentation:
  - Monitoring and alerting setup
  - Log aggregation
  - Performance tuning
  - Troubleshooting guide
- Architecture documentation updates:
  - Update `docs/ARCHITECTURE.md` with actual implementation
  - Sequence diagrams for request flow
  - Deployment architecture diagrams

**Acceptance Criteria:**
- [ ] Docker image builds and runs successfully
- [ ] docker-compose starts full stack (Aegis-MCP + Redis)
- [ ] Kubernetes manifests are production-ready
- [ ] All environment variables documented
- [ ] Troubleshooting guide covers common issues
- [ ] Architecture diagrams reflect implementation

**Complexity:** Low-Medium
**Estimated Time:** 10-12 hours

---

## Summary

**Total Issues:** 12
**Total Estimated Time:** 140-170 hours (3.5-4.25 weeks)

**Issue Breakdown by Phase:**
- Phase 1 (Foundation): 3 issues, 12-17 hours
- Phase 2 (Core Infrastructure): 5 issues, 58-66 hours
- Phase 3 (Core Caching Logic): 2 issues, 30-38 hours
- Phase 4 (MCP Protocol): 2 issues, 24-30 hours
- Phase 5 (Testing & Docs): 2 issues, 30-36 hours

**Critical Path:**
Issue #1 → Issue #2 → Issue #3 → Issue #4 → Issue #7 → Issue #9

**Parallel Work Opportunities:**
- Issue #5 can be done in parallel with Issue #4
- Issue #6 can be done in parallel with Issue #4
- Issue #8 can be started after Issue #7 is partially complete
- Issue #10 depends on Issue #9
- Issue #11 and #12 can be done in parallel

**Next Actions:**
1. Start with Issue #1 (Cargo workspace setup)
2. Set up development environment with Redis Stack
3. Create initial GitHub milestone with these issues
4. Begin implementation following the critical path
