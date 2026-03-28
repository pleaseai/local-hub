---
name: local-hub documentation structure
description: Documentation conventions, known issues, and structural patterns for local-hub repo
type: project
---

## Structure Conventions

- Root docs: `README.md`, `ARCHITECTURE.md`, `CLAUDE.md` — always valid orphan-exempt files
- `.please/INDEX.md` — central workspace navigation hub, always valid
- `.please/docs/knowledge/` — stable context docs (product.md, tech-stack.md, workflow.md, product-guidelines.md)
- `.please/docs/tracks/` — implementation tracks with spec.md + plan.md pairs
- `crates/server/CHANGELOG.md` — release history, linked from ARCHITECTURE.md Entry Points table

**Why:** This is the structure enforced by the `please` plugin workflow.

**How to apply:** When auditing links/orphans, treat the knowledge/ docs as implicitly referenced by the `.please` navigation system even without direct markdown links.

## Known Recurring Issues (resolved 2026-03-28)

- `stop` command documented but does not exist in code (only `start`, `status`, `flush`) — fixed in ARCHITECTURE.md, CLAUDE.md, product-guidelines.md
- README.md and ARCHITECTURE.md module listings were missing `proxy.rs`, `error.rs`, `lib.rs` — fixed
- Roadmap phases shown as uncompleted `[ ]` even after shipping — Phase 1 shipped in v0.3.0; Phase 4 (apps/web) in active development

## Version State (as of 2026-03-28)

- `crates/server` version: 0.3.0
- `[workspace.package]` version: 0.1.0 (not used by server crate — server overrides with own version)
- Phase 1 shipped; Phase 4 (apps/web) in active track `local-better-hub-20260327`
