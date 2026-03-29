# Entity Graph Cache

> Track: entity-graph-cache-20260329

## Overview

Replace the current parent-path cache invalidation with an entity graph-based system inspired by Stellate (GraphQL CDN). Track entity dependencies across cached responses and invalidate precisely when mutations occur. Additionally, enable GraphQL API caching by detecting query vs mutation in POST /graphql requests.

This moves local-hub from URL-pattern invalidation to semantic entity-level invalidation, enabling cross-protocol cache coherence between REST and GraphQL.

## Requirements

### Functional Requirements

- [ ] FR-1: Entity Extractor — Parse REST and GraphQL responses to extract entity identities using GitHub `node_id` (Global Relay ID). Top-level extraction only (response root entities).
- [ ] FR-2: Entity Registry — Maintain a reverse mapping of `entity_id → [cache_key]` in libsql. When an entity is referenced by a cached response, register the dependency.
- [ ] FR-3: REST Mutation Invalidation — On POST/PUT/PATCH/DELETE to REST endpoints, extract affected entity from the mutation response and invalidate all cache entries (REST + GraphQL) that reference that entity.
- [ ] FR-4: GraphQL Query Caching — Cache GraphQL query responses keyed by `SHA256(query + variables)`. Detect query vs mutation by parsing the operation type from the request body.
- [ ] FR-5: GraphQL Mutation Invalidation — On GraphQL mutation responses, extract affected entities and invalidate all related cache entries across both REST and GraphQL caches.
- [ ] FR-6: Cross-Protocol Invalidation — A REST mutation must invalidate related GraphQL caches, and a GraphQL mutation must invalidate related REST caches.
- [ ] FR-7: Storage Migration — Migrate cache storage from redb to libsql for both the response cache and the entity registry. Leverage libsql for SQL-based entity relationship queries and future Turso embedded replica support (Phase 3 team shared cache).

### Non-functional Requirements

- [ ] NFR-1: Cache lookup latency must remain under 5ms for local operations
- [ ] NFR-2: Entity extraction overhead must not add more than 2ms per response
- [ ] NFR-3: Storage size overhead for entity registry should be < 20% of response cache size
- [ ] NFR-4: Backward compatible — existing gh CLI and fetch/octokit clients require zero changes

## Acceptance Criteria

- [ ] AC-1: When a REST PATCH to `/repos/org/repo/issues/123` succeeds, both the REST `/issues/123` cache AND any GraphQL query referencing that issue's `node_id` are invalidated
- [ ] AC-2: GraphQL queries (POST /graphql with `query` operation) return cached responses with `x-local-hub-cache: hit` on repeated identical requests
- [ ] AC-3: GraphQL mutations (POST /graphql with `mutation` operation) pass through to GitHub and invalidate all cache entries referencing affected entities
- [ ] AC-4: Entity extraction correctly identifies `node_id` from both REST JSON responses and GraphQL response `data` fields
- [ ] AC-5: All existing REST caching behavior continues to work (TTL, ETag, token isolation)
- [ ] AC-6: libsql replaces redb as the storage backend with no data loss during migration

## Out of Scope

- Deep nested entity traversal (only top-level extraction)
- GraphQL subscription support
- Turso cloud replication (Phase 3)
- Automatic schema introspection for entity type detection
- Cache warming or prefetching

## Assumptions

- GitHub REST API responses consistently include `node_id` fields for primary entities
- GraphQL responses include entity identifiers in predictable locations (`id`, `node_id` fields)
- The `node_id` is stable and unique across REST and GraphQL for the same entity
- libsql Rust crate (`libsql`) is stable enough for production embedded use

## Technical Notes

### Entity Identity

GitHub provides `node_id` (Base64-encoded Global Relay ID) in both REST and GraphQL responses. This serves as the universal entity identifier:

```
REST: { "id": 123, "node_id": "MDExOlB1bGxSZXF1ZXN0MQ==", ... }
GraphQL: { "data": { "repository": { "issue": { "id": "MDExOlB1bGxSZXF1ZXN0MQ==" } } } }
```

### Storage Schema (libsql)

```sql
-- Response cache (replaces redb)
CREATE TABLE cache_entries (
  key TEXT PRIMARY KEY,
  etag TEXT,
  status INTEGER,
  headers TEXT,  -- JSON
  body BLOB,
  expires_at INTEGER
);

-- Entity registry (new)
CREATE TABLE entity_deps (
  entity_id TEXT NOT NULL,
  cache_key TEXT NOT NULL,
  PRIMARY KEY (entity_id, cache_key)
);
CREATE INDEX idx_entity ON entity_deps(entity_id);
CREATE INDEX idx_cache_key ON entity_deps(cache_key);
```

### GraphQL Operation Detection

```
POST /graphql { "query": "query { ... }" }  → cacheable
POST /graphql { "query": "mutation { ... }" } → pass-through + invalidate
```
