# local-hub — Tech Stack

## Language

- **Rust** (stable) — minimal memory, instant startup, single binary distribution

## Tooling

- **mise** — version management (rust = "stable")

## Core Dependencies

| Crate                            | Purpose                                                         |
| -------------------------------- | --------------------------------------------------------------- |
| `tokio`                          | Async runtime                                                   |
| `axum`                           | HTTP server framework                                           |
| `hyper`                          | Low-level HTTP (Unix socket support)                            |
| `hyper-util`                     | Hyper utilities (tokio integration)                             |
| `reqwest`                        | HTTP client (GitHub API forwarding)                             |
| `libsql`                         | Embedded SQLite (Turso fork, entity graph + future replication) |
| `serde` + `serde_json`           | JSON serialization                                              |
| `clap`                           | CLI argument parsing                                            |
| `tracing` + `tracing-subscriber` | Structured logging                                              |
| `sha2` + `hex`                   | Token hashing                                                   |
| `thiserror`                      | Error types                                                     |
| `http`                           | HTTP types                                                      |
| `tower`                          | Middleware layer                                                |

## Project Structure

Cargo workspace with `crates/` layout. Future web client crate planned.

```
crates/
  server/          # local-hub binary (proxy + CLI)
```

## CI/CD

- **GitHub Actions** — release workflow on main push
- **release-please** — automated versioning + CHANGELOG
- **Cross-compilation** — macOS (x64, arm64) + Linux (x64, arm64)
- **Homebrew** — `pleaseai/homebrew-tap` auto-updated on release

## Distribution

- `cargo install --git https://github.com/pleaseai/local-hub`
- `brew tap pleaseai/tap && brew install local-hub`
- GitHub Releases (prebuilt binaries)
