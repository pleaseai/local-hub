# Local Better Hub

> Track: local-better-hub-20260327

## Overview

Integrate vendor/better-hub's Web App (apps/web) into the local-hub project, modifying all GitHub API calls to route through the local-hub proxy (localhost:8787). Remove SaaS-specific features (Stripe billing), simplify authentication to GitHub OAuth + SQLite for a lightweight team-internal GitHub collaboration dashboard. Add local CLI-based AI providers (Claude Code, Gemini CLI, Codex CLI) to the AI Ghost feature.

## Requirements

### Functional Requirements

- [ ] FR-1: Copy vendor/better-hub/apps/web source to apps/web and configure boilerplate
- [ ] FR-2: Introduce `GITHUB_API_URL` environment variable (default: `https://api.github.com`)
- [ ] FR-3: Apply `GITHUB_API_URL` environment variable to Octokit instance `baseUrl`
- [ ] FR-4: Replace 30+ direct `fetch("https://api.github.com/...")` calls (REST + GraphQL) with `GITHUB_API_URL`-based URLs
- [ ] FR-5: Retain GitHub OAuth authentication (using better-auth layer)
- [ ] FR-6: Migrate DB from PostgreSQL → SQLite (change Prisma adapter)
- [ ] FR-7: Remove Stripe billing code and dependencies
- [ ] FR-8: Add `GITHUB_GRAPHQL_URL` environment variable (default: `https://api.github.com/graphql`)
- [ ] FR-9: Add AI SDK provider dependencies — `ai-sdk-provider-claude-code`, `ai-sdk-provider-gemini-cli`, `ai-sdk-provider-codex-cli`
- [ ] FR-10: Add local CLI provider options to AI Ghost mode selection (Claude Code / Gemini CLI / Codex CLI alongside OpenRouter)
- [ ] FR-11: Make AI provider configurable via environment variables or user settings

### Non-functional Requirements

- [ ] NFR-1: Setting `GITHUB_API_URL=https://api.github.com` must behave identically to original better-hub (backward compatible)
- [ ] NFR-2: Fallback to direct GitHub API calls when local-hub is not running
- [ ] NFR-3: Immediately runnable via `bun install && bun dev` after copy
- [ ] NFR-4: Graceful handling when AI provider CLI is not installed locally (fallback to OpenRouter)

## Acceptance Criteria

- [ ] AC-1: Setting `GITHUB_API_URL=http://localhost:8787` routes all GitHub API calls through local-hub
- [ ] AC-2: GitHub OAuth login succeeds and dashboard is accessible
- [ ] AC-3: Repo overview, PR review, and Issues pages render correctly
- [ ] AC-4: Stripe-related code completely removed (no imports/references)
- [ ] AC-5: SQLite DB migration succeeds and user data persists
- [ ] AC-6: AI Ghost generates responses via local Claude Code CLI when Claude Code provider is selected
- [ ] AC-7: AI Ghost works with Gemini CLI / Codex CLI providers

## Out of Scope

- Chrome/Firefox browser extensions (separate Track later)
- Sentry monitoring / Vercel Analytics (kept as-is)
- GraphQL API caching (not supported in local-hub Phase 1 — URLs converted to env vars for future readiness)
- Custom AI provider implementation (using ben-vargas open-source provider packages)

## Assumptions

- local-hub is running on localhost:8787 and transparently proxies GitHub REST API
- All team members have GitHub accounts
- bun is the package manager
- Prisma supports SQLite adapter
- Claude Code, Gemini CLI, Codex CLI are installed locally (OpenRouter fallback when not installed)
- ai-sdk-provider-claude-code, ai-sdk-provider-gemini-cli, ai-sdk-provider-codex-cli packages are available on npm registry

## References

- https://github.com/ben-vargas/ai-sdk-provider-claude-code
- https://github.com/ben-vargas/ai-sdk-provider-gemini-cli
- https://github.com/ben-vargas/ai-sdk-provider-codex-cli
