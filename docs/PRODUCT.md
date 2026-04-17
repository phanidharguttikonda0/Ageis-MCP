# PRODUCT.md — Aegis-MCP

## Summary

**Aegis-MCP** is a semantic caching server and governance layer designed to intercept traffic from agentic IDEs (like Cursor) to LLM providers. It acts as an intelligent proxy that uses vector embeddings to cache and semantically match similar prompts, reducing redundant LLM API calls while maintaining response quality through a high similarity threshold (0.95+ cosine distance).

## ICP (Ideal Customer Profile)

- **Company size:** 10–500 employees (development teams using AI-assisted IDEs)
- **Industry:** Software Development, DevTools, AI/ML Infrastructure
- **Geography:** Global (remote-first development teams)
- **Buyer role:** CTO, VP Engineering, Engineering Manager, Lead Developer

## User Roles

| Role | Description | Primary Use Cases |
|------|-------------|------------------|
| **Development Teams** | Engineers using AI-assisted IDEs (Cursor, etc.) | Reduce LLM API costs, improve response latency, maintain code privacy |
| **Platform Engineers** | Infrastructure team managing internal tools | Deploy and operate the caching layer, monitor cache hit rates, configure governance policies |
| **Engineering Leadership** | CTOs/VPs concerned about AI spending | Track AI usage patterns, enforce budget controls, optimize LLM spend |

## Pain Points

1. **Escalating LLM API Costs** - Development teams using AI-assisted IDEs generate massive volumes of redundant API calls for similar prompts
2. **Latency Issues** - Direct calls to LLM providers introduce latency that impacts developer productivity
3. **Lack of Governance** - No visibility or control over AI usage patterns, spending, or data privacy
4. **Redundant Computations** - Similar prompts are sent repeatedly to LLMs without caching mechanisms

## Competitive Context

- **Alternatives:** Direct LLM provider integration (no caching), simple HTTP caches (don't understand semantic similarity), manual prompt optimization
- **Key differentiators:** Semantic understanding via vector embeddings, MCP protocol integration for seamless IDE compatibility, governance and observability features

## Tech Stack

- **Languages:** Rust
- **Frameworks:** Axum (web routing), Tokio (async runtime)
- **Datastore:** Redis (with RediSearch/vector capabilities)
- **Protocols:** Model Context Protocol (MCP)
- **Observability:** tracing crate (deep instrumentation)
- **Dev command:** `cargo run`
- **Build command:** `cargo build`
- **Test command:** `cargo test`

## System Architecture

```
┌─────────────────┐     MCP Request      ┌──────────────────────┐
│  Agentic IDE    │ ──────────────────►  │   Aegis-MCP Server   │
│  (Cursor, etc.) │                      │   (Axum + Tokio)     │
└─────────────────┘                      └──────────┬───────────┘
                                                    │
                                                   ─┼─
                                      ┌─────────────▼─────────────┐
                                      │  Request Processing      │
                                      │  - Extract prompt text   │
                                      │  - Generate embedding    │
                                      └─────────────┬─────────────┘
                                                    │
                                      ┌─────────────▼─────────────┐
                                      │  Vector Similarity Search │
                                      │  (Redis + RediSearch)     │
                                      └─────────────┬─────────────┘
                                                    │
                              ┌─────────────────────┼─────────────────────┐
                              │                     │                     │
                         ┌────▼────┐         ┌─────▼──────┐       ┌─────▼──────┐
                         │ CACHE   │         │ CACHE      │       │  Forward   │
                         │ HIT     │         │ MISS       │       │  Upstream  │
                         │ (>0.95) │         │ (<0.95)    │       │  LLM API   │
                         └────┬────┘         └─────┬──────┘       └─────┬──────┘
                              │                     │                     │
                         ┌────▼────┐         ┌─────▼──────┐       ┌─────▼──────┐
                         │ Return  │         │ Async      │       │  Receive  │
                         │ Cached  │         │ Write to   │       │  Response  │
                         │ Response│         │ Redis      │       └─────┬──────┘
                         └────┬────┘         └─────┬──────┘             │
                              │                     │                     │
                              └─────────────────────┼─────────────────────┘
                                                    │
                                        ┌───────────▼───────────┐
                                        │  Return Response      │
                                        │  to IDE               │
                                        └───────────────────────┘
```

## Redis Vector Schema Design

### Hash Structure
```redis
HSET cache:<hash_id>
  prompt_text "<original prompt text>"
  embedding "<binary vector representation>"
  response_payload "<llm response json>"
  model_name "<llm model used>"
  created_at "<timestamp>"
  last_accessed "<timestamp>"
  access_count "<counter>"
  similarity_threshold "<float>"
```

### Vector Index Configuration
```redis
FT.CREATE cache_idx ON HASH PREFIX 1 cache:
  SCHEMA
    prompt_text TEXT
    embedding VECTOR HNSW 6
      TYPE FLOAT32
      DIM 1536
      DISTANCE_METRIC COSINE
      M 16
      EF_CONSTRUCTION 200
    model_name TAG
    created_at NUMERIC
    access_count NUMERIC
```

### Key Design Decisions
- **Cosine Distance** for vector similarity (appropriate for text embeddings)
- **HNSW algorithm** for efficient approximate nearest neighbor search
- **1536 dimensions** (OpenAI text-embedding-ada-002 standard)
- **Hash-based storage** for flexible schema evolution
- **Separate vector index** for performant similarity searches

## MCP Integration Strategy

### Protocol Compliance
Aegis-MCP will implement the Model Context Protocol (MCP) server specification:

1. **Transport Layer:** JSON-RPC 2.0 over stdio/HTTP (Axum router)
2. **Resource Exposing:** Expose cache statistics, configuration, and health endpoints
3. **Tool Integration:** Provide tools for cache management (invalidate, warm-up, stats)
4. **Prompt Templates:** Support templated prompts with automatic caching

### Request Interception Flow
```rust
// MCP request handler
async fn handle_mcp_request(request: MCPRequest) -> MCPResponse {
    // 1. Parse MCP request
    // 2. Extract prompt text
    // 3. Generate embedding (using local model or external embedding service)
    // 4. Query Redis for similar cached prompts
    // 5. If similarity > 0.95: return cached response
    // 6. Else: forward to upstream LLM, cache new entry
    // 7. Return MCP-formatted response
}
```

### IDE Compatibility
- **Primary Target:** Cursor IDE (MCP-native)
- **Secondary Targets:** VS Code (via MCP extension), JetBrains (via plugin)
- **Configuration:** Standard MCP server configuration JSON

## Concurrency Considerations

### High-Level Concurrency Strategy
1. **Async/Await Runtime:** Tokio for all I/O operations
2. **Connection Pooling:** Redis connection pool (bb8 or deadpool)
3. **Request Isolation:** Each request handled in independent Tokio task
4. **Lock-Free Design:** Minimize mutex usage, use channels for coordination

### Critical Path Performance
```
Request → Embedding Generation (10-50ms) → Redis Query (1-5ms) → Response
                                                   ↓
                                              Cache Hit (>0.95)
                                              Direct return (1-2ms)
```

### Concurrency Safety Mechanisms
- **Redis Transaction Safety:** WATCH/MULTI/EXEC for cache updates
- **Async Write-Back:** Cache misses write to Redis asynchronously (fire-and-forget)
- **Rate Limiting:** Per-client token bucket rate limiting
- **Circuit Breaker:** Upstream LLM API circuit breaker (fail-fast on degradation)
- **Request Timeouts:** Configurable timeouts at each stage (embedding, Redis, upstream)

### Scalability Points
- **Horizontal Scaling:** Stateless design allows multiple Aegis-MCP instances behind load balancer
- **Redis Clustering:** Support for Redis Cluster for vector index scaling
- **Embedding Generation:** Can offload to dedicated embedding service
- **Cache Eviction:** LRU eviction policy with configurable max memory

## Governance & Observability

### Logging Strategy
- **Structured Logging:** JSON-formatted logs via tracing crate
- **Request Tracing:** Distributed tracing IDs for request flow
- **Metrics:** OpenTelemetry metrics for cache hit/miss ratios, latency percentiles

### Monitoring Endpoints
```
GET /health          - Health check
GET /metrics         - Prometheus metrics
GET /stats/cache     - Cache statistics (hit rate, total requests, etc.)
GET /stats/latency   - Latency breakdown by stage
```

### Governance Controls
- **Budget Controls:** Maximum token limits per time period
- **Content Filtering:** Optional content moderation layer
- **Access Controls:** API key authentication for different teams
- **Audit Logging:** Full audit trail of all requests and responses

## Known Constraints

- **Similarity Threshold:** Fixed at 0.95 cosine similarity (configurable per deployment)
- **Embedding Model:** Requires external embedding service or local model integration
- **Redis Version:** Requires Redis Stack 7.2+ for vector search capabilities
- **Memory Requirements:** Vector index can be memory-intensive for large cache sizes
- **Network Latency:** Performance depends on low-latency Redis connection
