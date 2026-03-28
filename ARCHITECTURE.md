# Architecture

> Bird's-eye view of the local-hub codebase.
> See [CLAUDE.md](CLAUDE.md) for development instructions.

## Overview

local-hub is a transparent HTTP cache proxy for the GitHub API. It intercepts
GitHub API requests from any client (`gh` CLI, `fetch`, `octokit`), caches GET
responses locally, and serves them with near-zero latency on subsequent requests.

The proxy listens on two interfaces simultaneously:

- **Unix socket** — for `gh` CLI via its `http_unix_socket` config
- **TCP port** — for `fetch`, `octokit`, and other HTTP clients

Write requests (POST/PUT/PATCH/DELETE) pass through to GitHub with related cache
invalidation. Tokens are never stored — only truncated SHA-256 hashes serve as
cache namespace prefixes.

## Entry Points

| Path                            | Purpose                                       |
| ------------------------------- | --------------------------------------------- |
| `crates/server/src/main.rs`     | CLI entry point — parses args, starts server  |
| `crates/server/CHANGELOG.md`    | Release history — [view](crates/server/CHANGELOG.md) |
| `.github/workflows/release.yml` | CI/CD — release-please + cross-platform build |
| `Cargo.toml`                    | Workspace root — dependency versions          |

## Module Structure

```
crates/
└── server/               # @pleaseai/local-hub binary
    └── src/
        ├── main.rs        # CLI (clap) — start, status, flush
        ├── server.rs      # axum router + dual listener (Unix + TCP)
        ├── proxy.rs       # Request forwarding to GitHub API
        ├── cache.rs       # redb cache layer (get, set, invalidate)
        ├── key.rs         # Cache key generation (token hash + URL)
        ├── ttl.rs         # Per-endpoint TTL configuration
        ├── error.rs       # Typed error definitions
        └── lib.rs         # Library root (re-exports)
```

### Planned crates

| Crate    | Purpose                           | Status            |
| -------- | --------------------------------- | ----------------- |
| `server` | Proxy server + CLI                | Active            |
| `web`    | Dashboard client (via better-hub) | In progress (`apps/web`) |

## Data Flow

### Cache Hit (fast path)

```
Client → local-hub → redb lookup → cache hit + not expired → response
                                    ~0.3ms end-to-end
```

### Cache Miss

```
Client → local-hub → redb lookup → miss → reqwest → GitHub API
                                           ↓
                                    store response + ETag + TTL
                                           ↓
                                    response to client
```

### ETag Refresh (expired but cached)

```
Client → local-hub → redb lookup → expired → conditional request
                                    (If-None-Match: <etag>)
                                           ↓
                                    GitHub: 304 Not Modified
                                    (no rate limit cost)
                                           ↓
                                    refresh TTL → cached response
```

### Write Passthrough

```
Client → POST /repos/org/repo/issues → GitHub API
                                         ↓
                              invalidate repos:org:repo:* cache keys
                                         ↓
                              response to client
```

## Architecture Invariants

1. **Tokens are never persisted** — Only `SHA256(token)[:16]` used as cache key prefix.
   Violating this would create a security vulnerability.

2. **Fail open** — If redb is unavailable or corrupted, requests pass through to
   GitHub directly. The proxy must never block API access.

3. **GET-only caching** — Only GET requests are cached. All mutations pass through
   and trigger cache invalidation.

4. **Cache key isolation** — Different tokens MUST produce different cache namespaces.
   Serving cached data across token boundaries is a privilege escalation.

5. **Single binary** — No runtime dependencies. The release artifact is one
   statically-usable binary per platform.

## Cross-Cutting Concerns

### Error Handling

- `thiserror` for typed errors with `#[from]` conversions
- GitHub API errors pass through unchanged (status code, headers, body)
- Cache errors are logged and silently bypassed (fail open)

### Logging

- `tracing` with structured fields (method, url, cache_status, latency_ms)
- Default level: `warn`. Override: `RUST_LOG=local_hub=debug`

### Testing

- Unit tests per module (`#[cfg(test)]` inline)
- Integration tests in `tests/` with mock GitHub API server
- Coverage target: >80% for new code

### Configuration

All configuration via CLI flags with environment variable fallbacks:

| Flag          | Env                   | Default                       |
| ------------- | --------------------- | ----------------------------- |
| `--port`      | `LOCAL_HUB_PORT`      | `8787`                        |
| `--socket`    | `LOCAL_HUB_SOCKET`    | `~/.local-hub/local-hub.sock` |
| `--cache-dir` | `LOCAL_HUB_CACHE_DIR` | `~/.local-hub/cache`          |
| `--ttl`       | `LOCAL_HUB_TTL`       | `300` (seconds)               |

### Distribution

- **release-please** for automated versioning and CHANGELOG
- **GitHub Actions** cross-compiles for macOS (x64, arm64) + Linux (x64, arm64)
- **Homebrew** formula auto-updated via `pleaseai/homebrew-tap`
- **cargo install** from git for source builds
