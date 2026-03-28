# local-hub

Local GitHub API cache proxy — reduce latency and save rate limits.

## What it does

`local-hub` sits between your GitHub CLI (or any GitHub API client) and the GitHub API, transparently caching responses to deliver:

- **Near-zero latency** for repeated requests (served from local cache)
- **Rate limit savings** via ETag-based conditional requests (304 responses don't count against limits)
- **Offline access** to previously fetched data

```
gh CLI ──(Unix socket)──→ local-hub ──(HTTPS)──→ GitHub API
fetch  ──(HTTP :8787)───→  (cache)
```

## Features

- **Dual listen** — Unix socket for `gh` CLI + TCP port for `fetch`/`octokit`
- **ETag + TTL caching** — conditional requests for freshness, TTL for speed
- **Token-hash isolation** — each token gets its own cache namespace (no privilege leaks)
- **All GET requests cached** — issues, PRs, repos, orgs, projects, and more
- **Write passthrough** — POST/PUT/PATCH/DELETE go straight to GitHub with related cache invalidation
- **Tiny footprint** — ~3-5MB memory, <2ms cold start (Rust native binary)
- **Zero config** — sensible defaults, just start and go

## Installation

### Homebrew (macOS / Linux)

```bash
brew install pleaseai/tap/local-hub
```

### npm

```bash
npm install -g local-hub
```

### Cargo

```bash
cargo install --git https://github.com/pleaseai/local-hub
```

### Binary

Download platform-specific binaries from [Releases](https://github.com/pleaseai/local-hub/releases).

## Quick start

```bash
# Start the proxy
local-hub start

# Configure gh CLI to use it
gh config set http_unix_socket ~/.local-hub/local-hub.sock

# Use gh as normal — requests are now cached
gh issue list

# Show cache statistics
local-hub status

# Flush cache entries
local-hub flush

# For fetch/octokit clients, use HTTP endpoint
# baseUrl: http://localhost:8787
```

## How it works

### Caching strategy

1. **GET request arrives** → compute cache key from `SHA256(token)[:16] + method + URL + query`
2. **Cache hit + not expired** → return cached response instantly
3. **Cache hit + expired** → send conditional request with `If-None-Match: <etag>`
4. **304 Not Modified** → refresh TTL, return cached response (no rate limit cost)
5. **Cache miss or 200** → store response + ETag + TTL, return to client

### Cache invalidation

- **TTL-based** — configurable per-endpoint pattern (default: 5 minutes)
- **Write-triggered** — mutations automatically invalidate related cache entries
- **Manual** — `local-hub flush [pattern]` to clear specific entries

### Authentication

Tokens are **never stored**. Only a truncated SHA-256 hash is used as a cache namespace prefix, ensuring:

- Each token's cached data is isolated
- Token rotation automatically creates a fresh namespace
- No risk of serving cached data across different permission levels

## Configuration

```bash
# Custom port
local-hub start --port 9000

# Custom cache directory
local-hub start --cache-dir ~/.my-cache

# Custom default TTL (seconds)
local-hub start --ttl 600

# Show cache stats
local-hub status
```

## Architecture

```
crates/
└── server/
    └── src/
        ├── main.rs       # CLI entry point (start, status, flush)
        ├── server.rs     # axum HTTP proxy (TCP + Unix socket)
        ├── proxy.rs      # Request forwarding to GitHub API
        ├── cache.rs      # redb cache layer (get, set, invalidate)
        ├── key.rs        # Cache key generation (token hash + URL normalization)
        ├── ttl.rs        # Per-endpoint TTL rules
        ├── error.rs      # Typed error definitions
        └── lib.rs        # Library root (re-exports)
```

### Tech stack

- **Language**: [Rust](https://www.rust-lang.org) — minimal memory, instant startup
- **HTTP**: [axum](https://github.com/tokio-rs/axum) + [hyper](https://hyper.rs) — async HTTP server
- **Cache**: [redb](https://github.com/cberner/redb) — embedded key-value store (single file, ACID)
- **Tooling**: [mise](https://mise.jdx.dev) — version management

## Roadmap

- [x] **Phase 1**: Local proxy with TTL + ETag caching
- [ ] **Phase 2**: Webhook-based cache invalidation (via relay-worker)
- [ ] **Phase 3**: Team shared cache (L2 layer)
- [ ] **Phase 4**: Web client dashboard (via better-hub) — in progress (`apps/web`)

## License

MIT
