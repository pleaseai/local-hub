# local-hub

Local GitHub API cache proxy — reduce latency and save rate limits.

## What it does

`local-hub` sits between your GitHub CLI (or any GitHub API client) and the GitHub API, transparently caching responses to deliver:

- **Near-zero latency** for repeated requests (served from local cache)
- **Rate limit savings** via ETag-based conditional requests (304 responses don't count against limits)
- **Offline access** to previously fetched data

```
gh CLI ──→ local-hub (localhost) ──→ GitHub API
              │
              └─ unstorage cache (fs)
```

## Features

- **Transparent HTTP proxy** — set `GITHUB_API_URL` and everything just works
- **ETag + TTL caching** — conditional requests for freshness, TTL for speed
- **Token-hash isolation** — each token gets its own cache namespace (no privilege leaks)
- **All GET requests cached** — issues, PRs, repos, orgs, projects, and more
- **Write passthrough** — POST/PUT/PATCH/DELETE go straight to GitHub with related cache invalidation
- **Zero config** — sensible defaults, just start and go

## Quick start

```bash
# Install
bun add -g @pleaseai/local-hub

# Start the proxy
local-hub start

# Configure gh CLI to use it
export GITHUB_API_URL=http://localhost:8787

# Use gh as normal — requests are now cached
gh issue list
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
src/
├── cli.ts          # CLI entry point (start, stop, status, flush)
├── server.ts       # Bun HTTP proxy server
├── cache.ts        # unstorage cache layer (get, set, invalidate)
├── key.ts          # Cache key generation (token hash + URL normalization)
└── ttl.ts          # Per-endpoint TTL rules
```

### Tech stack

- **Runtime**: [Bun](https://bun.sh) — fast startup, built-in HTTP server
- **Cache**: [unstorage](https://unstorage.unjs.io) with fs driver — swappable storage backend
- **Zero compile step** — TypeScript executed directly by Bun

## Roadmap

- [ ] **Phase 1**: Local proxy with TTL + ETag caching
- [ ] **Phase 2**: Webhook-based cache invalidation (via relay-worker)
- [ ] **Phase 3**: Team shared cache (Cloudflare Worker + D1 as L2)

## License

MIT
