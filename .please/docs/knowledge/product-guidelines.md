# local-hub — Product Guidelines

## Design Principles

1. **Transparency first** — Users should not need to change their workflow. Configure once, then use `gh` / `fetch` as normal.
2. **Security by default** — Tokens are never stored. Only truncated SHA-256 hashes used as cache namespace prefixes.
3. **Fail open** — If the cache is unavailable, requests pass through to GitHub API directly. Never block the user.
4. **Minimal dependencies** — Rust native binary with no runtime dependencies. Single binary distribution.
5. **Sensible defaults** — Works out of the box with zero configuration. Advanced users can tune TTL, port, cache directory.

## CLI UX

- Commands follow standard Unix conventions: `local-hub start`, `local-hub stop`, `local-hub status`, `local-hub flush`
- Quiet by default — only log errors. Use `--verbose` for debug output.
- Exit codes: 0 = success, 1 = error, 2 = already running (for `start`)

## Error Handling

- Network errors to GitHub: return cached data if available (stale-while-error), otherwise proxy the error
- Cache corruption: log warning, delete corrupted entry, fetch fresh from GitHub
- Invalid tokens: pass through GitHub's 401 response without caching

## Logging

- Use `tracing` with structured logging
- Default level: `warn`
- Environment variable: `RUST_LOG=local_hub=debug` for verbose output
