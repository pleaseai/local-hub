use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libsql::{Connection, Database};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::storage;

/// A cached HTTP response entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub etag: Option<String>,
    pub cached_at: u64,
    pub expires_at: u64,
}

impl CacheEntry {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= self.expires_at
    }

    pub fn refresh_ttl(&mut self, ttl: Duration) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.expires_at = now + ttl.as_secs();
    }
}

/// Entity-aware cache backed by libsql.
/// Manages response caching and entity dependency tracking in a single database.
pub struct EntityAwareCache {
    db: Database,
    conn: Connection,
}

impl EntityAwareCache {
    /// Open or create a cache database at the given path.
    pub async fn open(path: &Path) -> Result<Self> {
        let db = storage::open_database(path).await?;
        let conn = storage::connect(&db)?;
        Ok(Self { db, conn })
    }

    /// Create from an existing database (for testing or shared access).
    pub fn from_database(db: Database, conn: Connection) -> Self {
        Self { db, conn }
    }

    /// Get a new connection to the underlying database.
    pub fn connect(&self) -> Result<Connection> {
        storage::connect(&self.db)
    }

    /// Get a cache entry by key.
    pub async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let mut rows = self
            .conn
            .query(
                "SELECT status, headers, body, etag, cached_at, expires_at \
                 FROM cache_entries WHERE key = ?1",
                [key],
            )
            .await?;

        match rows.next().await? {
            Some(row) => {
                let status = row.get::<i64>(0)? as u16;
                let headers_json = row.get::<String>(1)?;
                let body = row.get::<Vec<u8>>(2)?;
                let etag: Option<String> = row.get::<String>(3).ok();
                let cached_at = row.get::<i64>(4)? as u64;
                let expires_at = row.get::<i64>(5)? as u64;

                let headers: Vec<(String, String)> = serde_json::from_str(&headers_json)?;

                Ok(Some(CacheEntry {
                    status,
                    headers,
                    body,
                    etag,
                    cached_at,
                    expires_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Store a cache entry with associated entity IDs.
    /// Replaces existing entry and entity deps for the same key.
    pub async fn store(&self, key: &str, entry: &CacheEntry, entity_ids: &[String]) -> Result<()> {
        let headers_json = serde_json::to_string(&entry.headers)?;

        // Upsert cache entry
        self.conn
            .execute(
                "INSERT OR REPLACE INTO cache_entries \
                 (key, status, headers, body, etag, cached_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                libsql::params![
                    key,
                    entry.status as i64,
                    headers_json,
                    entry.body.clone(),
                    entry.etag.clone(),
                    entry.cached_at as i64,
                    entry.expires_at as i64,
                ],
            )
            .await?;

        // Replace entity deps: remove old, insert new
        self.conn
            .execute("DELETE FROM entity_deps WHERE cache_key = ?1", [key])
            .await?;

        for entity_id in entity_ids {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO entity_deps (entity_id, cache_key) \
                     VALUES (?1, ?2)",
                    libsql::params![entity_id.as_str(), key],
                )
                .await?;
        }

        Ok(())
    }

    /// Remove a cache entry by exact key and clean up entity deps.
    pub async fn remove(&self, key: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM cache_entries WHERE key = ?1", [key])
            .await?;
        self.conn
            .execute("DELETE FROM entity_deps WHERE cache_key = ?1", [key])
            .await?;
        Ok(affected > 0)
    }

    /// Remove all entries whose keys start with the given prefix.
    pub async fn remove_by_prefix(&self, prefix: &str) -> Result<usize> {
        // SQLite LIKE with escaped prefix + '%'
        let pattern = format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"));

        // Get keys to remove (for entity dep cleanup)
        let mut rows = self
            .conn
            .query(
                "SELECT key FROM cache_entries WHERE key LIKE ?1 ESCAPE '\\'",
                [pattern.as_str()],
            )
            .await?;

        let mut keys = Vec::new();
        while let Some(row) = rows.next().await? {
            keys.push(row.get::<String>(0)?);
        }

        let count = keys.len();
        if count > 0 {
            for key in &keys {
                self.conn
                    .execute(
                        "DELETE FROM entity_deps WHERE cache_key = ?1",
                        [key.as_str()],
                    )
                    .await?;
            }
            self.conn
                .execute(
                    "DELETE FROM cache_entries WHERE key LIKE ?1 ESCAPE '\\'",
                    [pattern.as_str()],
                )
                .await?;
        }

        Ok(count)
    }

    /// Invalidate all cache entries referencing any of the given entity IDs.
    /// Returns the number of cache entries removed.
    pub async fn invalidate_by_entities(&self, entity_ids: &[String]) -> Result<usize> {
        if entity_ids.is_empty() {
            return Ok(0);
        }

        // Find all cache keys referencing these entities
        // Collect all cache keys referencing any of the given entities
        let mut keys = Vec::new();
        for entity_id in entity_ids {
            let mut rows = self
                .conn
                .query(
                    "SELECT DISTINCT cache_key FROM entity_deps WHERE entity_id = ?1",
                    [entity_id.as_str()],
                )
                .await?;
            while let Some(row) = rows.next().await? {
                let key = row.get::<String>(0)?;
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }

        let count = keys.len();
        for key in &keys {
            self.conn
                .execute("DELETE FROM cache_entries WHERE key = ?1", [key.as_str()])
                .await?;
            self.conn
                .execute(
                    "DELETE FROM entity_deps WHERE cache_key = ?1",
                    [key.as_str()],
                )
                .await?;
        }

        Ok(count)
    }

    /// Count total entries in the cache.
    pub async fn count(&self) -> Result<usize> {
        let mut rows = self
            .conn
            .query("SELECT count(*) FROM cache_entries", ())
            .await?;
        let row = rows.next().await?.expect("count should return a row");
        Ok(row.get::<i64>(0)? as usize)
    }

    /// Update an existing cache entry (e.g., refresh TTL after 304).
    pub async fn update(&self, key: &str, entry: &CacheEntry) -> Result<()> {
        let headers_json = serde_json::to_string(&entry.headers)?;
        self.conn
            .execute(
                "UPDATE cache_entries SET status = ?1, headers = ?2, body = ?3, \
                 etag = ?4, cached_at = ?5, expires_at = ?6 WHERE key = ?7",
                libsql::params![
                    entry.status as i64,
                    headers_json,
                    entry.body.clone(),
                    entry.etag.clone(),
                    entry.cached_at as i64,
                    entry.expires_at as i64,
                    key,
                ],
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_cache() -> (EntityAwareCache, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-cache.db");
        let cache = EntityAwareCache::open(&db_path).await.unwrap();
        (cache, dir)
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn sample_entry(ttl_secs: u64) -> CacheEntry {
        let now = now_secs();
        CacheEntry {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: br#"{"id":1}"#.to_vec(),
            etag: Some("\"abc123\"".into()),
            cached_at: now,
            expires_at: now + ttl_secs,
        }
    }

    #[tokio::test]
    async fn test_get_missing_key() {
        let (cache, _dir) = temp_cache().await;
        let result = cache.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);
        cache.store("test:key", &entry, &[]).await.unwrap();

        let retrieved = cache.get("test:key").await.unwrap().unwrap();
        assert_eq!(retrieved.status, 200);
        assert_eq!(retrieved.body, br#"{"id":1}"#);
        assert_eq!(retrieved.etag, Some("\"abc123\"".into()));
    }

    #[tokio::test]
    async fn test_entry_not_expired() {
        let entry = sample_entry(300);
        assert!(!entry.is_expired());
    }

    #[tokio::test]
    async fn test_entry_expired() {
        let now = now_secs();
        let entry = CacheEntry {
            status: 200,
            headers: vec![],
            body: vec![],
            etag: None,
            cached_at: now - 600,
            expires_at: now - 1,
        };
        assert!(entry.is_expired());
    }

    #[tokio::test]
    async fn test_refresh_ttl() {
        let now = now_secs();
        let mut entry = CacheEntry {
            status: 200,
            headers: vec![],
            body: vec![],
            etag: None,
            cached_at: now - 600,
            expires_at: now - 1,
        };
        assert!(entry.is_expired());
        entry.refresh_ttl(Duration::from_secs(300));
        assert!(!entry.is_expired());
        assert!(entry.expires_at > now);
    }

    #[tokio::test]
    async fn test_remove() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);
        cache.store("test:remove", &entry, &[]).await.unwrap();

        assert!(cache.remove("test:remove").await.unwrap());
        assert!(cache.get("test:remove").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let (cache, _dir) = temp_cache().await;
        assert!(!cache.remove("does_not_exist").await.unwrap());
    }

    #[tokio::test]
    async fn test_remove_by_prefix() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);

        cache
            .store("abc:GET:/repos/org/repo/issues", &entry, &[])
            .await
            .unwrap();
        cache
            .store("abc:GET:/repos/org/repo/pulls", &entry, &[])
            .await
            .unwrap();
        cache
            .store("abc:GET:/repos/other/repo/issues", &entry, &[])
            .await
            .unwrap();
        cache
            .store("xyz:GET:/repos/org/repo/issues", &entry, &[])
            .await
            .unwrap();

        let removed = cache
            .remove_by_prefix("abc:GET:/repos/org/repo")
            .await
            .unwrap();
        assert_eq!(removed, 2);
        assert_eq!(cache.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let (cache, _dir) = temp_cache().await;
        assert_eq!(cache.count().await.unwrap(), 0);

        let entry = sample_entry(300);
        cache.store("key1", &entry, &[]).await.unwrap();
        cache.store("key2", &entry, &[]).await.unwrap();
        assert_eq!(cache.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_overwrite_entry() {
        let (cache, _dir) = temp_cache().await;
        let entry1 = sample_entry(300);
        cache.store("key", &entry1, &[]).await.unwrap();

        let mut entry2 = sample_entry(600);
        entry2.status = 304;
        cache.store("key", &entry2, &[]).await.unwrap();

        let retrieved = cache.get("key").await.unwrap().unwrap();
        assert_eq!(retrieved.status, 304);
    }

    #[tokio::test]
    async fn test_store_with_entities() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);
        let entities = vec!["MDExOlB1bGxSZXF1ZXN0MQ==".to_string()];
        cache.store("test:key", &entry, &entities).await.unwrap();

        // Verify entity dep was registered
        let conn = cache.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT cache_key FROM entity_deps WHERE entity_id = ?1",
                ["MDExOlB1bGxSZXF1ZXN0MQ=="],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "test:key");
    }

    #[tokio::test]
    async fn test_invalidate_by_entities() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);

        // Store entries with shared entity
        let entity = vec!["ENTITY_A".to_string()];
        cache.store("rest:key1", &entry, &entity).await.unwrap();
        cache.store("gql:key2", &entry, &entity).await.unwrap();
        cache.store("rest:key3", &entry, &[]).await.unwrap(); // no entity

        assert_eq!(cache.count().await.unwrap(), 3);

        let removed = cache
            .invalidate_by_entities(&["ENTITY_A".to_string()])
            .await
            .unwrap();
        assert_eq!(removed, 2);
        assert_eq!(cache.count().await.unwrap(), 1);

        // key3 should still exist
        assert!(cache.get("rest:key3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_invalidate_empty_entities() {
        let (cache, _dir) = temp_cache().await;
        let removed = cache.invalidate_by_entities(&[]).await.unwrap();
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn test_entity_deps_replaced_on_overwrite() {
        let (cache, _dir) = temp_cache().await;
        let entry = sample_entry(300);

        cache
            .store("key", &entry, &["OLD_ENTITY".to_string()])
            .await
            .unwrap();
        cache
            .store("key", &entry, &["NEW_ENTITY".to_string()])
            .await
            .unwrap();

        // OLD_ENTITY should no longer reference key
        let removed = cache
            .invalidate_by_entities(&["OLD_ENTITY".to_string()])
            .await
            .unwrap();
        assert_eq!(removed, 0);

        // NEW_ENTITY should reference key
        let removed = cache
            .invalidate_by_entities(&["NEW_ENTITY".to_string()])
            .await
            .unwrap();
        assert_eq!(removed, 1);
    }
}
