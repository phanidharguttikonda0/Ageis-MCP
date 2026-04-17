# Aegis-MCP

**Aegis-MCP** is a semantic caching server and governance layer designed to intercept traffic from agentic IDEs (like Cursor) to LLM providers. It uses vector embeddings to intelligently cache and semantically match similar prompts, dramatically reducing redundant LLM API costs while maintaining response quality through a high similarity threshold (0.95+ cosine distance).

## Architecture Overview

Aegis-MCP acts as an intelligent proxy that:
1. **Intercepts** LLM prompts via the Model Context Protocol (MCP)
2. **Generates** vector embeddings of prompt text
3. **Queries** Redis using Vector Similarity Search (cosine distance)
4. **Returns** cached responses if similarity > 0.95 (cache hit)
5. **Forwards** to upstream LLM on cache miss, asynchronously writing new entries

## Technology Stack

- **Language:** Rust (memory safety, zero-cost abstractions)
- **Framework:** Axum (routing) + Tokio (async runtime)
- **Datastore:** Redis with RediSearch (vector similarity search)
- **Protocol:** Model Context Protocol (MCP) for IDE integration
- **Observability:** tracing crate (deep instrumentation)

## Getting Started

### Prerequisites

- Rust 2024 edition
- Redis Stack 7.2+ (for vector search capabilities)
- OpenAI API key (or alternative embedding/LLM provider)

### Installation

```bash
# Clone the repository
git clone https://github.com/phanidharguttikonda0/Ageis-MCP.git
cd Ageis-MCP

# Build the project
cargo build --release

# Start Redis Stack (using Docker)
docker run -d -p 6379:6379 redis/redis-stack-server:latest

# Configure environment
cp .env.example .env
# Edit .env with your API keys and configuration

# Run the server
cargo run
```

### Development

```bash
# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Lint code
cargo clippy

# Run all checks
cargo test && cargo clippy
```

## Configuration

See `.env.example` for full configuration options. Key settings:

- `REDIS_URL` - Redis connection string
- `CACHE_SIMILARITY_THRESHOLD` - Default: 0.95
- `LLM_UPSTREAM_URL` - Upstream LLM API endpoint
- `OPENAI_API_KEY` - Your OpenAI API key

## Documentation

- **[Product Brief](docs/PRODUCT.md)** - System architecture, user roles, technical specifications
- **[Architecture](docs/ARCHITECTURE.md)** - Detailed system design and component documentation
- **[Implementation Guide](docs/IMPLEMENTATION.md)** - Code organization and development patterns
- **[Milestone 1 Issues](.vibekit/MILESTONE_1_ISSUES.md)** - Detailed implementation roadmap

## Current Status

🚧 **Under Active Development** - This project is in early alpha stage. Core infrastructure is being implemented per [Milestone 1](.vibekit/MILESTONE_1_ISSUES.md).

## Contributing

Please read our implementation guide and code organization documentation before contributing. All code should:
- Pass `cargo clippy` with no warnings
- Have unit tests with >80% coverage
- Include comprehensive tracing instrumentation
- Follow Rust best practices and idioms

## License

[Specify your license here]
