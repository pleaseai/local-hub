# local-hub

Local GitHub API cache proxy — reduce latency and save rate limits.

## Tech Stack

- **Runtime**: Bun (packageManager: bun@1.3.10)
- **Monorepo**: Turborepo + bun workspaces
- **Cache**: unstorage (fs driver)
- **Lint**: @antfu/eslint-config + husky + lint-staged + commitlint (conventional)

## Project Structure

```
packages/
  server/             # @pleaseai/local-hub — HTTP proxy server + CLI
    src/
      cli.ts          # CLI entry point (start, stop, status, flush)
      server.ts       # Bun HTTP proxy server
      cache.ts        # unstorage cache layer
      key.ts          # Cache key generation (token hash + URL normalization)
      ttl.ts          # Per-endpoint TTL rules
```

## Development Commands

```bash
bun install                    # Install dependencies
bun run dev                    # Dev mode (turbo)
bun run build                  # Build (turbo)
bun run lint:fix               # Lint fix (turbo)
```
