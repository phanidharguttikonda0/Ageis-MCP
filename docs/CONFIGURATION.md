# Configuration Management

Aegis-MCP uses a sophisticated configuration management system that supports multiple sources with clear priority ordering, validation, and security features.

## Configuration Sources (Priority Order)

Configuration is loaded from multiple sources in order of priority (highest to lowest):

1. **Environment Variables** - Override all other sources
2. **Config Files** - `config/production.toml` or `config/development.toml`
3. **Default Values** - Built-in fallback values

## Environment Variables

All environment variables use the `AEGIS_` prefix with double underscore (`__`) as separator for nested values.

### Server Configuration
```bash
export AEGIS_SERVER__HOST="0.0.0.0"
export AEGIS_SERVER__PORT=8080
export AEGIS_SERVER__MAX_CONNECTIONS=10000
```

### Redis Configuration
```bash
export AEGIS_REDIS__URL="redis://localhost:6379"
export AEGIS_REDIS__POOL_SIZE=50
export AEGIS_REDIS__CONNECTION_TIMEOUT=5000
export AEGIS_REDIS__MAX_LIFETIME=3600
```

### Cache Configuration
```bash
export AEGIS_CACHE__SIMILARITY_THRESHOLD=0.95
export AEGIS_CACHE__MAX_CACHE_SIZE=100000
export AEGIS_CACHE__TTL_SECONDS=604800
```

### LLM Configuration
```bash
export AEGIS_LLM__UPSTREAM_URL="https://api.openai.com/v1"
export AEGIS_LLM__API_KEY="sk-..."  # Required
export AEGIS_LLM__MODEL="gpt-4"
export AEGIS_LLM__TIMEOUT=30000
export AEGIS_LLM__MAX_RETRIES=3
```

### Observability Configuration
```bash
export AEGIS_OBSERVABILITY__LOG_LEVEL="info"
export AEGIS_OBSERVABILITY__JAEGER_ENDPOINT="http://localhost:4318"
```

## Config Files

Configuration files are stored in the `config/` directory with TOML format:

### Development Configuration (`config/development.toml`)
```toml
[server]
host = "127.0.0.1"
port = 8080
max_connections = 100

[redis]
url = "redis://localhost:6379"
pool_size = 5
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
log_level = "debug"
jaeger_endpoint = "http://localhost:4318"
```

### Production Configuration (`config/production.toml`)
```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 10000

[redis]
url = "${REDIS_URL}"
pool_size = 50
connection_timeout = 5000
max_lifetime = 3600

[cache]
similarity_threshold = 0.95
max_cache_size = 100000
ttl_seconds = 604800  # 7 days

[llm]
upstream_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
model = "gpt-4"
timeout = 30000
max_retries = 3

[observability]
log_level = "info"
jaeger_endpoint = "http://jaeger:4318"
```

## Environment Selection

Set the `ENVIRONMENT` environment variable to select which config file to use:

```bash
export ENVIRONMENT=development  # Uses config/development.toml
export ENVIRONMENT=production   # Uses config/production.toml
```

If not set, defaults to `development`.

## Configuration Validation

The system validates all configuration on startup and fails fast with clear error messages:

- **Server port** cannot be 0
- **Max connections** cannot be 0
- **Redis URL** cannot be empty
- **Redis pool size** cannot be 0
- **Cache similarity threshold** must be between 0.0 and 1.0
- **Max cache size** cannot be 0
- **LLM upstream URL** cannot be empty
- **LLM API key** cannot be empty (required)
- **LLM model** cannot be empty
- **LLM timeout** cannot be 0
- **Log level** must be one of: trace, debug, info, warn, error

## Secret Redaction

For security, all secrets are automatically redacted from logs:

```
llm: LLMConfig { upstream_url: "https://api.openai.com/v1", api_key: "***REDACTED***", model: "gpt-4", timeout: 30000, max_retries: 3 }
```

## Default Values

If no configuration is provided, these defaults are used:

| Setting | Default Value |
|---------|---------------|
| Server host | 127.0.0.1 |
| Server port | 8080 |
| Max connections | 1000 |
| Redis URL | redis://localhost:6379 |
| Redis pool size | 10 |
| Cache similarity threshold | 0.95 |
| Max cache size | 10000 |
| Cache TTL | 86400 seconds (24 hours) |
| LLM upstream URL | https://api.openai.com/v1 |
| LLM model | gpt-4 |
| LLM timeout | 30000ms (30 seconds) |
| LLM max retries | 3 |
| Log level | info |
| Jaeger endpoint | http://localhost:4318 |

## Examples

### Minimal Configuration (with API key only)
```bash
export AEGIS_LLM__API_KEY="sk-..."
cargo run -p aegis-mcp
```

### Production Configuration
```bash
export ENVIRONMENT=production
export REDIS_URL="redis://production-redis:6379"
export OPENAI_API_KEY="sk-..."
export AEGIS_OBSERVABILITY__LOG_LEVEL="warn"
cargo run -p aegis-mcp --features production
```

### Development with Debug Logging
```bash
export ENVIRONMENT=development
export OPENAI_API_KEY="sk-..."
export AEGIS_OBSERVABILITY__LOG_LEVEL="debug"
RUST_LOG=debug cargo run -p aegis-mcp --features development
```

## Configuration File Placeholders

Config files support environment variable substitution using `${VAR_NAME}` syntax:

```toml
[llm]
api_key = "${OPENAI_API_KEY}"  # Substituted at runtime
```

## Testing Configuration

The configuration system includes comprehensive tests:

```bash
# Run configuration tests
cargo test -p aegis-core config

# Test with different environments
ENVIRONMENT=production cargo test -p aegis-core config
```

## Troubleshooting

### Configuration Loading Failures

If you see configuration errors, check:

1. **API Key Required**: `LLM API key cannot be empty. Set AEGIS_LLM__API_KEY environment variable`
   - Solution: `export AEGIS_LLM__API_KEY="your-api-key"`

2. **Invalid Port**: `Server port cannot be 0`
   - Solution: Set a valid port (1-65535)

3. **Invalid Threshold**: `Cache similarity threshold must be between 0.0 and 1.0`
   - Solution: Use a value between 0.0 and 1.0

4. **Invalid Log Level**: `Invalid log level 'xxx'. Must be one of: [...]`
   - Solution: Use one of: trace, debug, info, warn, error

### Configuration File Not Found

If config files are missing, the system will use defaults and environment variables. This is expected behavior and not an error.

### Priority Conflicts

Remember the priority: Environment Variables > Config Files > Defaults

If you set both `AEGIS_SERVER__PORT=3000` and have `port = 8080` in your config file, the environment variable (3000) will be used.

## Security Best Practices

1. **Never commit API keys** to version control
2. **Use environment variables** for secrets in production
3. **Use config files** for non-sensitive settings
4. **Enable secret redaction** - it's automatic
5. **Rotate credentials regularly** and update environment variables
6. **Use different API keys** for development and production

## Next Steps

After configuration is set up:
1. Configure Redis Stack (vector search capabilities)
2. Set up observability (Jaeger, Prometheus)
3. Test cache operations
4. Deploy with production configuration
