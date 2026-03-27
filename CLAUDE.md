# local-hub

Local GitHub API cache proxy — reduce latency and save rate limits.

## References

- [ARCHITECTURE.md](ARCHITECTURE.md) — Bird's-eye view of the codebase
- [.please/INDEX.md](.please/INDEX.md) — Workspace navigation (tracks, specs, decisions)

## Tech Stack

- **Language**: Rust (stable)
- **Async runtime**: tokio
- **HTTP server**: axum + hyper (TCP + Unix socket dual listen)
- **HTTP client**: reqwest (GitHub API forwarding)
- **Cache storage**: redb (embedded key-value store)
- **CLI**: clap
- **Tooling**: mise (version management)

## Project Structure

```
crates/
  server/               # local-hub binary — HTTP proxy server + CLI
    src/
      main.rs           # Entry point + CLI (start, stop, status, flush)
```

## Monorepo

Cargo workspace. 추후 web client crate 추가 예정.

## Development Commands

```bash
mise install                   # Install toolchain
cargo build                    # Build
cargo run -- start             # Run server
cargo test                     # Test
cargo clippy                   # Lint
cargo fmt                      # Format
```

## Architecture

```
gh CLI ──(Unix socket)──→ local-hub ──(HTTPS)──→ GitHub API
fetch  ──(HTTP :8787)───→  (cache)
```

- gh CLI: `http_unix_socket` config으로 연결
- fetch/octokit: `http://localhost:8787` baseUrl로 연결
- Cache key: `SHA256(token)[:16] + method + URL + query`
- Invalidation: TTL + ETag conditional requests
