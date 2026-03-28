# local-hub — Product Definition

## Vision

Eliminate GitHub API latency and rate limit friction for developers by providing a transparent local cache proxy that sits between any GitHub API client and the GitHub API.

## Core Value

- **Near-zero latency**: Repeated requests served from local cache instantly
- **Rate limit savings**: ETag-based conditional requests (304 responses cost 0 rate limit)
- **Offline access**: Previously fetched data available without network
- **Transparent**: No client code changes needed — configure once and forget
- **Tiny footprint**: ~3-5MB memory, <2ms cold start (Rust native binary)

## Target Users

- **CLI power users**: Heavy `gh` CLI users hitting rate limits
- **AI agents**: `gh please` and similar automation tools making repeated API calls
- **App developers**: Applications using `@octokit` or raw `fetch` against GitHub API
- **Teams**: Shared cache layer for organizations (future phase)

## Product Scope

### Supported Clients

| Client              | Connection Method                       |
| ------------------- | --------------------------------------- |
| `gh` CLI            | Unix socket (`http_unix_socket` config) |
| `fetch` / `octokit` | HTTP TCP port (localhost:8787)          |
| `curl`              | Either method                           |

### Caching Strategy

1. All GET requests cached transparently
2. ETag + TTL-based freshness (configurable per-endpoint)
3. Write requests (POST/PUT/PATCH/DELETE) pass through + invalidate related cache
4. Token-hash isolation — no privilege leaks between different tokens

### Out of Scope (Phase 1)

- Team shared cache (Phase 3)
- Web dashboard (Phase 4)
- GitHub Enterprise Server support
- GraphQL API caching (REST only in Phase 1)

## Roadmap

- **Phase 1**: Local proxy with TTL + ETag caching — shipped (v0.3.0)
- **Phase 2**: Webhook-based cache invalidation (via relay-worker)
- **Phase 3**: Team shared cache (Cloudflare Worker + D1 as L2)
- **Phase 4**: Web client dashboard (via better-hub) — in progress (`apps/web`)
