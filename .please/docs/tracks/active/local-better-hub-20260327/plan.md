# Plan: Local Better Hub

> Track: local-better-hub-20260327
> Spec: [spec.md](./spec.md)

## Overview

- **Source**: .please/docs/tracks/active/local-better-hub-20260327/spec.md
- **Issue**: TBD
- **Created**: 2026-03-27
- **Approach**: Pragmatic (copy-then-modify)

## Purpose

After this change, team members can run a local Better Hub dashboard via `bun dev` to browse GitHub repos, PRs, and issues through the local-hub proxy. They can select Claude Code, Gemini CLI, or Codex CLI as local AI providers in the Ghost assistant for code review and PR analysis. Setting `GITHUB_API_URL=http://localhost:8787` and accessing the dashboard verifies all API calls route through local-hub.

## Context

better-hub is a GitHub collaboration dashboard built with Next.js 16 + better-auth + Prisma + OpenRouter AI. It currently operates as a SaaS product assuming PostgreSQL, Stripe billing, and Vercel deployment. Converting it to a local team-internal tool requires modifications across 5 areas.

GitHub API calls are hardcoded in 10 files at 38 locations (9 Octokit instances + 24 GraphQL fetch calls + 1 REST constant + 4 API route Octokits). Stripe is already conditionally loaded (`isStripeEnabled`) but referenced across 22 files including billing UI, Inngest cron jobs, and token usage tracking. The database uses PostgreSQL + pg.Pool + PrismaPg adapter with Decimal types and Serializable isolation. AI Ghost calls OpenRouter via Vercel AI SDK `streamText()` with BYOK (Bring Your Own Key) support and billing integration.

### Non-Goals

- Chrome/Firefox browser extensions
- GraphQL API caching (not supported in local-hub Phase 1)
- Custom AI provider implementation (using ben-vargas open-source packages)
- Removing Sentry/Vercel Analytics (kept as-is)

## Architecture Decision

**Copy-then-modify** approach selected. Copy vendor/better-hub/apps/web to apps/web, then modify 5 areas sequentially.

For GitHub API URL changes, create a central config module (`lib/github-config.ts`) exporting `GITHUB_API_URL` and `GITHUB_GRAPHQL_URL`, with a `createConfiguredOctokit(token)` helper to centralize Octokit creation. Replace all 24 GraphQL fetch URLs with the constant.

For DB migration, change Prisma provider to sqlite, remove pg.Pool/PrismaPg adapter, and convert Decimal to Float. For Stripe removal, leverage conditional loading to remove related imports/code/dependencies. For AI providers, use Vercel AI SDK's provider pattern with a factory function for routing.

## Tasks

### Phase 1: Source Copy & Boilerplate

- [ ] T001 Copy vendor/better-hub/apps/web to apps/web and remove unnecessary files (file: apps/web)
- [ ] T002 Add apps/web workspace to root package.json and verify bun install (file: package.json)

### Phase 2: GitHub API URL Environment Variables

- [ ] T003 Create GitHub API config module — GITHUB_API_URL, GITHUB_GRAPHQL_URL, createConfiguredOctokit() (file: apps/web/src/lib/github-config.ts, depends on T001)
- [ ] T004 Replace 9 Octokit instances with createConfiguredOctokit() (file: apps/web/src/lib/github.ts, depends on T003)
- [ ] T005 Replace 24 GraphQL fetch URLs with GITHUB_GRAPHQL_URL constant (file: apps/web/src/lib/github.ts, depends on T003)
- [ ] T006 Replace OG data module GitHub API URL with environment variable (file: apps/web/src/lib/og/og-data.ts, depends on T003)
- [ ] T007 Replace 4 Octokit instances in API routes (file: apps/web/src/app/api, depends on T003)

### Phase 3: DB Migration (PostgreSQL → SQLite)

- [ ] T008 Change Prisma schema provider and convert Decimal → Float (file: apps/web/prisma/schema.prisma, depends on T001)
- [ ] T009 Rewrite db.ts — remove pg.Pool/PrismaPg, add SQLite connection (file: apps/web/src/lib/db.ts, depends on T008)
- [ ] T010 Update prisma.config.ts for SQLite provider (file: apps/web/prisma.config.ts, depends on T008)
- [ ] T011 Change prismaAdapter provider to "sqlite" in auth.ts (file: apps/web/src/lib/auth.ts, depends on T009)

### Phase 4: Stripe Removal

- [ ] T012 Remove Stripe plugin and related imports from auth.ts (file: apps/web/src/lib/auth.ts, depends on T011)
- [ ] T013 Remove stripeClient plugin from auth-client.ts (file: apps/web/src/lib/auth-client.ts, depends on T001)
- [ ] T014 [P] Remove Stripe-specific code from lib/billing/ — stripe.ts, credit.ts, spending-limit.ts, config.ts Stripe references (file: apps/web/src/lib/billing, depends on T012)
- [ ] T015 [P] Remove 3 billing API routes (file: apps/web/src/app/api/billing, depends on T012)
- [ ] T016 [P] Remove subscription/payment UI from billing-tab.tsx (file: apps/web/src/components/settings/tabs/billing-tab.tsx, depends on T012)
- [ ] T017 Remove Stripe payment validation from usage-limit.ts — local always allowed (file: apps/web/src/lib/billing/usage-limit.ts, depends on T014)
- [ ] T018 Remove Stripe meter reporting from token-usage.ts — keep logging only (file: apps/web/src/lib/billing/token-usage.ts, depends on T014)
- [ ] T019 Remove stripe, @better-auth/stripe dependencies from package.json (file: apps/web/package.json, depends on T018)
- [ ] T020 Remove Stripe retry logic from Inngest cron job (file: apps/web/src/lib/inngest.ts, depends on T014)

### Phase 5: AI Provider Addition

- [ ] T021 Add ai-sdk-provider-claude-code, ai-sdk-provider-gemini-cli, ai-sdk-provider-codex-cli dependencies (file: apps/web/package.json, depends on T019)
- [ ] T022 Create AI provider factory module — route by provider type (file: apps/web/src/lib/ai-providers.ts, depends on T021)
- [ ] T023 Integrate provider factory in ghost/route.ts — support CLI providers alongside OpenRouter (file: apps/web/src/app/api/ai/ghost/route.ts, depends on T022)
- [ ] T024 Add AI provider selection field to user settings (file: apps/web/src/lib/user-settings-store.ts, depends on T022)
- [ ] T025 Add CLI provider options to AI model tab UI (file: apps/web/src/components/settings/tabs/ai-model-tab.tsx, depends on T024)

### Phase 6: Integration & Verification

- [ ] T026 Create .env.example and document environment variables (file: apps/web/.env.example, depends on T025)
- [ ] T027 Verify bun install && bun dev and fix build errors (depends on T026)
- [ ] T028 Run Prisma migrate dev and verify SQLite DB creation (depends on T027)
- [ ] T029 E2E verification — GitHub OAuth login + dashboard access (depends on T028)

## Key Files

### Create

- `apps/web/src/lib/github-config.ts` — Central GitHub API URL configuration module
- `apps/web/src/lib/ai-providers.ts` — AI provider factory (OpenRouter + CLI providers)
- `apps/web/.env.example` — Environment variable template

### Modify

- `apps/web/src/lib/github.ts` (7,413 lines) — 1 Octokit + 24 GraphQL fetch URL changes
- `apps/web/src/lib/auth.ts` — Stripe plugin removal + Octokit baseUrl + SQLite provider
- `apps/web/src/lib/db.ts` — pg.Pool → SQLite migration
- `apps/web/prisma/schema.prisma` — provider + Decimal type changes
- `apps/web/src/app/api/ai/ghost/route.ts` — provider factory integration
- `apps/web/src/lib/billing/usage-limit.ts` — Stripe validation removal
- `apps/web/src/lib/billing/token-usage.ts` — Stripe meter removal
- `apps/web/src/lib/user-settings-store.ts` — AI provider field addition
- `apps/web/src/components/settings/tabs/ai-model-tab.tsx` — CLI provider UI
- `apps/web/package.json` — dependency additions/removals

### Reuse

- `apps/web/src/lib/auth-plugins/pat-signin.ts` — PAT auth (kept, Octokit baseUrl only)
- `apps/web/src/lib/ai-auth.ts` — AI auth (kept, Octokit baseUrl only)

## Verification

### Automated Tests

- [ ] github-config.ts default and custom environment variable values
- [ ] createConfiguredOctokit() correctly sets baseUrl
- [ ] AI provider factory routes by provider type

### Observable Outcomes

- After setting `GITHUB_API_URL=http://localhost:8787` and accessing the dashboard, local-hub logs show incoming requests
- After running `bun dev`, navigating to http://localhost:3000 shows the login page
- Running `grep -r "stripe" apps/web/src/ | wc -l` returns 0

### Manual Testing

- [ ] GitHub OAuth login followed by dashboard access
- [ ] Repo overview page shows file tree and README
- [ ] PR review page shows diff
- [ ] AI Ghost generates response with Claude Code provider selected
- [ ] Settings page allows AI provider change

### Acceptance Criteria Check

- [ ] AC-1: GITHUB_API_URL=http://localhost:8787 routes all GitHub API calls through local-hub
- [ ] AC-2: GitHub OAuth login succeeds and dashboard is accessible
- [ ] AC-3: Repo overview, PR review, Issues pages render correctly
- [ ] AC-4: Stripe-related code completely removed (no imports/references)
- [ ] AC-5: SQLite DB migration succeeds
- [ ] AC-6: AI Ghost works with Claude Code provider
- [ ] AC-7: AI Ghost works with Gemini CLI / Codex CLI providers

## Decision Log

- Decision: Copy-then-modify approach selected
  Rationale: Managing independently is more suitable for local-only modifications than forking the vendor submodule
  Date/Author: 2026-03-27 / Claude

- Decision: Centralize Octokit creation in a single helper
  Rationale: Eliminates duplicate code across 9 locations; future baseUrl changes managed from a single point
  Date/Author: 2026-03-27 / Claude

- Decision: Keep token usage logging when removing Stripe
  Rationale: AI usage monitoring is useful even without billing; only Stripe meter reporting removed
  Date/Author: 2026-03-27 / Claude

## Outcomes & Retrospective

### What Was Shipped

- Local Better Hub dashboard (apps/web) — vendor/better-hub copy with local-hub proxy routing
- Configurable GitHub API URL via GITHUB_API_URL / GITHUB_GRAPHQL_URL env vars (38 locations, 14 files)
- SQLite database replacing PostgreSQL (Prisma schema + adapter migration)
- Stripe billing fully removed (auth plugin, billing files, API routes, cron jobs, UI, dependencies)
- CLI AI providers (Claude Code, Gemini CLI, Codex CLI) integrated in Ghost assistant

### What Went Well

- Central github-config.ts module cleanly consolidated all GitHub API URL references
- Stripe removal was straightforward thanks to existing conditional loading (isStripeEnabled)
- AI SDK provider factory pattern is extensible for future providers
- TypeScript type checking passes after all modifications

### What Could Improve

- Pre-existing vendor issues (Redis cache key mismatch, shell injection in sandbox, empty error handlers) should be addressed
- Prisma migration for SQLite needs testing (migration files are still PostgreSQL-formatted)
- E2E testing with actual GitHub OAuth and local-hub proxy not yet performed

### Tech Debt Created

- Pre-existing Redis cache key mismatch in auth.ts (read uses raw token, write uses hash)
- Pre-existing shell injection risk in E2B sandbox git config (commitAuthor)
- PostgreSQL migration SQL files in prisma/migrations/ need regeneration for SQLite
- billing-tab.tsx is a stub — could be removed entirely or repurposed for usage stats
