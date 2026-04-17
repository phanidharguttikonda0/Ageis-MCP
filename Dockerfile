# Multi-stage Dockerfile for Aegis-MCP
# Stage 1: Build
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    curl

# Set working directory
WORKDIR /build

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the application
RUN cargo build --release --bins

# Stage 2: Runtime
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    openssl \
    curl

# Create non-root user
RUN addgroup -g 1000 aegis && \
    adduser -D -u 1000 -G aegis aegis

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /build/target/release/aegis-mcp /app/aegis-mcp

# Create necessary directories
RUN mkdir -p /app/config && \
    mkdir -p /app/logs && \
    chown -R aegis:aegis /app

# Switch to non-root user
USER aegis

# Expose default port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Set default environment variables
ENV RUST_LOG=info \
    AEGIS_SERVER_HOST=0.0.0.0 \
    AEGIS_SERVER_PORT=8080 \
    AEGIS_REDIS_URL=redis://redis:6379 \
    AEGIS_CACHE_SIMILARITY_THRESHOLD=0.95 \
    AEGIS_CACHE_TTL_SECONDS=3600 \
    AEGIS_LLM_UPSTREAM_URL=https://api.openai.com/v1 \
    AEGIS_LLM_MODEL=gpt-4

# Run the application
CMD ["./aegis-mcp"]