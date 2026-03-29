# Plan: Entity Graph Cache

> Track: entity-graph-cache-20260329
> Spec: [spec.md](./spec.md)

## Overview

- **Source**: .please/docs/tracks/active/entity-graph-cache-20260329/spec.md
- **Issue**: #9
- **Created**: 2026-03-29
- **Approach**: Unified Rewrite

## Purpose

After this change, developers using local-hub will get precise, entity-level cache invalidation across both REST and GraphQL APIs. They can verify it works by issuing a REST PATCH to an issue and confirming that both the REST cache entry and any GraphQL query referencing that issue's `node_id` are invalidated automatically.

## Context

local-hub is a transparent GitHub API cache proxy that sits between clients (gh CLI, fetch/octokit) and the GitHub API. It currently caches only REST GET requests using redb (an embedded key-value store), with TTL + ETag freshness and parent-path prefix invalidation on mutations. GraphQL requests (POST /graphql) bypass the cache entirely because all GraphQL operations use POST, which the proxy treats as mutations.

The current parent-path invalidation has limitations: it only invalidates one level up, misses related resources, and cannot handle cross-protocol scenarios. This track replaces the entire storage and invalidation layer with an entity graph-based system inspired by Stellate (GraphQL CDN). Entity dependencies are tracked via GitHub's `node_id` (Global Relay ID), enabling precise cross-protocol cache invalidation. The storage backend migrates from redb to libsql for SQL-based entity relationship queries and future Turso embedded replica support (Phase 3 team shared cache).

### Requirements (from spec)

- FR-1 through FR-7: Entity extraction, entity registry, REST/GraphQL invalidation, cross-protocol invalidation, libsql migration
- NFR-1: Lookup < 5ms, NFR-2: Extraction overhead < 2ms
- NFR-4: Zero client-side changes

### Constraints

- Single-binary distribution must be preserved (libsql embeds SQLite)
- Fail-open invariant: if entity extraction fails, fall back to prefix invalidation
- Token isolation: entity registry is namespaced by token hash
- File size limit: 500 LOC per source file

### Non-Goals

- Deep nested entity traversal (top-level extraction only)
- GraphQL subscriptions
- Turso cloud replication (Phase 3)
- Schema introspection for entity type detection
- Cache warming or prefetching

## Architecture Decision

**Chosen: Unified Rewrite with EntityAwareCache facade.**

Replace redb entirely with libsql. Introduce `EntityAwareCache` as the single facade that handles response caching and entity dependency registration in a single SQL transaction. The proxy handler is restructured with a `RequestClassifier` that categorizes every request into one of four kinds (REST GET, REST mutation, GraphQL query, GraphQL mutation), then feeds into a shared pipeline.

Rejected alternatives:

- **Incremental (layer-by-layer)**: Lower risk but leaves legacy patterns. Parent-path invalidation bolted alongside entity graph creates two invalidation paths that are hard to reason about. Temporary state where entity deps are stored but not used adds unnecessary intermediate complexity.
- **Trait abstraction over backends**: Over-engineered for a single-backend project with no plan to support multiple storage backends.

Key design principles:

- Entity extraction is synchronous in the store path (not async/background) to guarantee consistency.
- Prefix invalidation preserved as fallback when `node_id` is absent from a response.
- All cache operations are async since libsql provides a native async API; all callers are already in async context.
- libsql schema uses two tables: `cache_entries` (response cache) and `entity_deps` (entity→cache_key reverse mapping).

### Data Flow

```
                    ┌──────────────────────────────────────┐
                    │            proxy_handler              │
                    └──────────┬───────────────────────────┘
                               │ classify request
              ┌────────────────┼────────────────┐
              ▼                ▼                 ▼
         REST GET        POST /graphql      REST Mutation
              │          (classify.rs)           │
              │         ┌────┴────┐              │
              │     GQL Query  GQL Mutation      │
              ▼         ▼         │              ▼
     entity_cache    entity_cache │     forward_to_github()
        .get()        .get()      │              │
              │         │         │         response body
         hit? ► resp    │         │              │
         miss?          │         │              ▼
              ▼         ▼         ▼     extract_entity_ids()
     fetch_from_github()  forward()              │
              │         │         │              ▼
              ▼         ▼         ▼  entity_cache.invalidate_by_entities()
     extract_entity_ids()         │     + remove_by_prefix() [fallback]
              │         │         │
              ▼         ▼         ▼
     entity_cache      entity_cache  extract_entity_ids()
        .store()        .store()      │
              │         │              ▼
              ▼         ▼     invalidate_by_entities()
         response    response         │
                                      ▼
                                 response
```

### Storage Schema (libsql)

```sql
-- Response cache (replaces redb key-value)
CREATE TABLE IF NOT EXISTS cache_entries (
  key        TEXT PRIMARY KEY,
  status     INTEGER NOT NULL,
  headers    TEXT NOT NULL,      -- JSON array of [name, value] pairs
  body       BLOB NOT NULL,
  etag       TEXT,
  cached_at  INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

-- Entity dependency registry
CREATE TABLE IF NOT EXISTS entity_deps (
  entity_id TEXT NOT NULL,
  cache_key TEXT NOT NULL,
  PRIMARY KEY (entity_id, cache_key)
);
CREATE INDEX IF NOT EXISTS idx_entity_deps_entity ON entity_deps(entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_deps_key ON entity_deps(cache_key);
```

### Module Structure

```
crates/server/src/
  lib.rs              (modify: add new module declarations)
  storage.rs          (new: libsql connection, schema DDL)
  entity_cache.rs     (new: EntityAwareCache facade)
  entity.rs           (new: entity extraction from JSON)
  classify.rs         (new: request classification)
  proxy.rs            (rewrite: unified entity-aware handler)
  key.rs              (modify: add graphql_cache_key)
  error.rs            (modify: replace redb errors with libsql)
  server.rs           (modify: async AppState construction)
  ttl.rs              (modify: add /graphql TTL rule)
  main.rs             (modify: async cache init, .db path)
```

## Tasks

### Phase 1: Storage Foundation

- [x] T001 Create libsql storage module with connection management and schema DDL (file: crates/server/src/storage.rs)
- [x] T002 [P] Replace error types from redb to libsql (file: crates/server/src/error.rs) (depends on T001)
- [x] T003 Build EntityAwareCache facade with async CRUD + entity registration (file: crates/server/src/entity_cache.rs) (depends on T001, T002)

### Phase 2: Entity & GraphQL Modules

- [x] T004 [P] Create entity extractor for REST and GraphQL responses (file: crates/server/src/entity.rs)
- [x] T005 [P] Create request classifier for REST/GraphQL routing (file: crates/server/src/classify.rs)
- [x] T006 [P] Add GraphQL cache key generation (file: crates/server/src/key.rs) (depends on T001)

### Phase 3: Proxy Rewrite

- [x] T007 Rewrite proxy handler with unified entity-aware flow (file: crates/server/src/proxy.rs) (depends on T003, T004, T005, T006)
- [x] T008 Update server wiring and AppState for EntityAwareCache (file: crates/server/src/server.rs) (depends on T003, T007)

### Phase 4: Integration & Cleanup

- [x] T009 Update lib.rs exports and main.rs initialization (file: crates/server/src/main.rs) (depends on T007, T008)
- [x] T010 Remove old cache.rs, update Cargo.toml dependencies (file: crates/server/Cargo.toml) (depends on T009)
- [x] T011 Add integration tests for cross-protocol invalidation (file: crates/server/tests/integration.rs) (depends on T009)
- [x] T012 Add GraphQL-specific TTL rules (file: crates/server/src/ttl.rs) (depends on T007)

## Key Files

### Create

- `crates/server/src/storage.rs` — libsql connection pool, schema DDL, migration helpers. ~80 LOC.
- `crates/server/src/entity_cache.rs` — EntityAwareCache facade: transactional cache+entity ops. ~180 LOC.
- `crates/server/src/entity.rs` — Parse REST/GraphQL JSON bodies for `node_id` fields. ~120 LOC.
- `crates/server/src/classify.rs` — Categorize request into RestGet/RestMutation/GqlQuery/GqlMutation. ~80 LOC.
- `crates/server/tests/integration.rs` — Cross-protocol invalidation integration tests. ~150 LOC.

### Modify

- `crates/server/src/proxy.rs` — Rewrite with unified classifier + entity-aware cache pipeline. ~350 LOC.
- `crates/server/src/key.rs` — Add `graphql_cache_key()`, make `token_hash()` pub(crate). ~130 LOC.
- `crates/server/src/error.rs` — Swap 6 redb error variants for libsql error variant. ~20 LOC.
- `crates/server/src/server.rs` — Async AppState construction with EntityAwareCache. ~70 LOC.
- `crates/server/src/main.rs` — Async cache init, update DEFAULT_CACHE_DIR to `.local-hub/cache.db`. ~165 LOC.
- `crates/server/src/lib.rs` — Add new module declarations, update exports. ~15 LOC.
- `crates/server/src/ttl.rs` — Add `/graphql` TTL rule. ~60 LOC.
- `crates/server/Cargo.toml` — Replace `redb` with `libsql`.
- `Cargo.toml` — Add `libsql` to workspace dependencies.

### Reuse

- `crates/server/src/key.rs` → `cache_key()` — REST cache key generation (unchanged)
- `crates/server/src/key.rs` → `invalidation_prefix()` — Prefix invalidation kept as fallback
- `crates/server/src/ttl.rs` → `TtlConfig::resolve()` — TTL resolution for all requests
- `crates/server/src/proxy.rs` → `parse_github_response()` — Response parsing reused for GraphQL
- `crates/server/src/proxy.rs` → `cache_entry_to_response()` / `github_response_to_axum()` — Response builders

### Delete

- `crates/server/src/cache.rs` — Replaced entirely by `entity_cache.rs` + `storage.rs`

## Interfaces and Dependencies

### New dependency

`libsql = "0.6"` (embedded SQLite with Turso extensions; single-file database)

Note: libsql's async API means all cache operations become `async`. Since `proxy_handler` is already async and all callers are in async context, this is a natural fit. The `main.rs` CLI commands (`status`, `flush`) will need `.await` but are already inside `#[tokio::main]`.

### EntityAwareCache (`entity_cache.rs`)

```rust
pub struct EntityAwareCache {
    conn: libsql::Connection,
}

impl EntityAwareCache {
    pub async fn open(path: &Path) -> Result<Self>;
    pub async fn get(&self, key: &str) -> Result<Option<CacheEntry>>;
    pub async fn store(&self, key: &str, entry: &CacheEntry, entity_ids: &[String]) -> Result<()>;
    pub async fn remove(&self, key: &str) -> Result<bool>;
    pub async fn remove_by_prefix(&self, prefix: &str) -> Result<usize>;
    pub async fn invalidate_by_entities(&self, entity_ids: &[String]) -> Result<usize>;
    pub async fn count(&self) -> Result<usize>;
}
```

### EntityExtractor (`entity.rs`)

```rust
/// Extract GitHub node_id values from a REST JSON response body.
pub fn extract_entity_ids(body: &[u8]) -> Vec<String>;

/// Extract entity IDs from a GraphQL response body.
pub fn extract_graphql_entity_ids(body: &[u8]) -> Vec<String>;
```

### RequestClassifier (`classify.rs`)

```rust
pub enum RequestKind {
    RestGet,
    RestMutation,
    GraphqlQuery { query: String, variables: Option<serde_json::Value> },
    GraphqlMutation,
}

pub fn classify(method: &str, path: &str, body: Option<&[u8]>) -> RequestKind;
```

### GraphQL cache key (`key.rs` addition)

```rust
/// Generate a cache key for a GraphQL query.
/// Format: {token_hash}:GQL:{sha256(query + variables_json)}
pub fn graphql_cache_key(token: &str, query: &str, variables: Option<&str>) -> String;
```

## Verification

### Automated Tests

- [ ] EntityAwareCache CRUD: get, store, remove, count (mirrors existing 10 cache.rs tests)
- [ ] Entity registration: store entry with entities, verify entity_deps populated
- [ ] Entity invalidation: invalidate by entity_id removes all referencing cache entries
- [ ] Cross-protocol invalidation: REST mutation invalidates GraphQL cache entries
- [ ] GraphQL query caching: identical queries return cache hit
- [ ] GraphQL mutation detection: mutations bypass cache and trigger invalidation
- [ ] Request classifier: correctly categorizes REST GET, REST mutation, GraphQL query, GraphQL mutation
- [ ] Entity extractor: extracts `node_id` from REST and GraphQL response bodies
- [ ] Prefix fallback: responses without `node_id` still invalidate via prefix matching
- [ ] Key generation: `graphql_cache_key` produces deterministic keys for same query+variables

### Observable Outcomes

- After `PATCH /repos/org/repo/issues/123`, a subsequent `GET /repos/org/repo/issues/123` returns `x-local-hub-cache: miss`
- After `POST /graphql` with a query, a repeat request returns `x-local-hub-cache: hit`
- After `POST /graphql` with a mutation on issue#123, a cached GraphQL query referencing that issue returns `x-local-hub-cache: miss`
- Running `local-hub status` shows cache entry count from libsql database

### Manual Testing

- [ ] Start local-hub, make REST requests, verify caching works as before
- [ ] Make a GraphQL query, verify it caches on repeat
- [ ] Mutate via REST, verify GraphQL cache for same entity is invalidated
- [ ] Mutate via GraphQL, verify REST cache for same entity is invalidated

### Acceptance Criteria Check

- [ ] AC-1: REST PATCH to issue invalidates both REST and GraphQL caches referencing that issue's node_id
- [ ] AC-2: GraphQL queries return `x-local-hub-cache: hit` on repeated identical requests
- [ ] AC-3: GraphQL mutations pass through and invalidate all cache entries referencing affected entities
- [ ] AC-4: Entity extraction correctly identifies `node_id` from REST and GraphQL responses
- [ ] AC-5: All existing REST caching behavior continues to work (TTL, ETag, token isolation)
- [ ] AC-6: libsql replaces redb with no data loss during migration

## Progress

- [x] (2026-03-29 17:45 KST) T001 Create libsql storage module
      Evidence: `cargo test --lib` → 3 storage tests passed
- [x] (2026-03-29 17:45 KST) T002 Replace error types from redb to libsql
- [x] (2026-03-29 17:45 KST) T003 Build EntityAwareCache facade
      Evidence: `cargo test --lib` → 14 entity_cache tests passed (CRUD + entity deps + invalidation)
- [x] (2026-03-29 17:45 KST) T004 Create entity extractor
      Evidence: `cargo test --lib` → 9 entity tests passed
- [x] (2026-03-29 17:45 KST) T005 Create request classifier
      Evidence: `cargo test --lib` → 10 classify tests passed
- [x] (2026-03-29 17:45 KST) T006 Add GraphQL cache key generation
      Evidence: `cargo test --lib` → 12 key tests passed (8 existing + 4 new graphql)
- [x] (2026-03-29 17:45 KST) T007 Rewrite proxy handler
      Evidence: `cargo test --lib` → 4 proxy tests passed
- [x] (2026-03-29 17:45 KST) T008 Update server wiring
- [x] (2026-03-29 17:45 KST) T009 Update lib.rs + main.rs
- [x] (2026-03-29 17:45 KST) T010 Remove cache.rs, update Cargo.toml
- [x] (2026-03-29 17:45 KST) T011 Update integration tests
- [x] (2026-03-29 17:45 KST) T012 Add GraphQL TTL rule
      Evidence: `cargo test --lib` → 62 total tests passed, clippy clean

## Decision Log

- Decision: Unified Rewrite over Incremental approach
  Rationale: Entity graph is fundamentally different from prefix invalidation; bolting it onto existing code creates two parallel invalidation paths. Clean rewrite with EntityAwareCache facade is more maintainable. The proxy handler is small enough (~390 LOC) that a full rewrite is manageable in a single pass.
  Date/Author: 2026-03-29 / User + Claude

- Decision: libsql over rusqlite for storage
  Rationale: libsql (Turso) provides embedded replica support for Phase 3 team shared cache. API-compatible with SQLite but with future extensibility.
  Date/Author: 2026-03-29 / User

- Decision: GitHub node_id as entity identity
  Rationale: node_id (Global Relay ID) is present in both REST and GraphQL responses, providing a universal cross-protocol entity identifier without additional mapping.
  Date/Author: 2026-03-29 / User

- Decision: Top-level entity extraction only
  Rationale: Deep nested traversal adds complexity and latency (NFR-2: <2ms extraction overhead). Top-level covers the primary use case; can be extended later.
  Date/Author: 2026-03-29 / User

- Decision: Synchronous entity extraction in store path
  Rationale: Entity extraction is a JSON parse of the already-buffered response body (~1ms for typical GitHub responses). Running it synchronously ensures entity deps are registered before the next request can read the cache entry. Async background registration creates a race condition window. NFR-2 budget is 2ms, well within range.
  Date/Author: 2026-03-29 / Claude

## Notes

### Risk Assessment

| Risk                                  | Likelihood | Impact | Mitigation                                                                       |
| ------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------- |
| libsql crate instability              | Medium     | High   | Pin exact version; comprehensive tests; fail-open to prefix invalidation         |
| Binary size increase (~3MB)           | Certain    | Low    | Acceptable for functionality gained; single-binary preserved                     |
| proxy.rs rewrite regression           | Medium     | High   | Existing test coverage (5 unit + integration); incremental verification per task |
| `node_id` missing from some endpoints | Medium     | Low    | Fall back to prefix invalidation; log at debug level                             |
| GraphQL query parsing false positives | Low        | Medium | Conservative detection: require `query`/`mutation` keyword at start              |
| Transaction contention under load     | Low        | Medium | libsql WAL mode by default; entity registration is fast INSERT OR IGNORE         |

### Module Size Budget

| File            | Current LOC | Estimated Post-Rewrite | Within 500 LOC |
| --------------- | ----------- | ---------------------- | -------------- |
| storage.rs      | new         | ~80                    | Yes            |
| entity_cache.rs | new         | ~180                   | Yes            |
| entity.rs       | new         | ~120                   | Yes            |
| classify.rs     | new         | ~80                    | Yes            |
| proxy.rs        | 389         | ~350                   | Yes            |
| key.rs          | 99          | ~130                   | Yes            |
| error.rs        | 35          | ~20                    | Yes            |
