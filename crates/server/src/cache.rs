use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::error::Result;

const CACHE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("cache");

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

/// Cache store backed by redb (embedded key-value database).
pub struct CacheStore {
    db: Database,
}

impl CacheStore {
    /// Open or create a cache database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(path)?;

        // Ensure the table exists
        let write_txn = db.begin_write()?;
        {
            let _table = write_txn.open_table(CACHE_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    /// Get a cache entry by key. Returns None if not found.
    pub fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CACHE_TABLE)?;

        match table.get(key)? {
            Some(value) => {
                let entry: CacheEntry = serde_json::from_slice(value.value())?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Store a cache entry.
    pub fn set(&self, key: &str, entry: &CacheEntry) -> Result<()> {
        let serialized = serde_json::to_vec(entry)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CACHE_TABLE)?;
            table.insert(key, serialized.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Remove a cache entry by exact key.
    #[allow(dead_code)]
    pub fn remove(&self, key: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(CACHE_TABLE)?;
            table.remove(key)?.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// Remove all entries whose keys start with the given prefix.
    /// Returns the number of entries removed.
    pub fn remove_by_prefix(&self, prefix: &str) -> Result<usize> {
        let keys_to_remove = {
            let read_txn = self.db.begin_read()?;
            let table = read_txn.open_table(CACHE_TABLE)?;
            let mut keys = Vec::new();
            for entry in table.iter()? {
                let (key, _) = entry?;
                if key.value().starts_with(prefix) {
                    keys.push(key.value().to_string());
                }
            }
            keys
        };

        let count = keys_to_remove.len();
        if count > 0 {
            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE)?;
                for key in &keys_to_remove {
                    table.remove(key.as_str())?;
                }
            }
            write_txn.commit()?;
        }
        Ok(count)
    }

    /// Count total entries in the cache.
    pub fn count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CACHE_TABLE)?;
        Ok(table.len()? as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_cache() -> (CacheStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-cache.redb");
        let store = CacheStore::open(&db_path).unwrap();
        (store, dir)
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

    #[test]
    fn test_get_missing_key() {
        let (store, _dir) = temp_cache();
        let result = store.get("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_set_and_get() {
        let (store, _dir) = temp_cache();
        let entry = sample_entry(300);
        store.set("test:key", &entry).unwrap();

        let retrieved = store.get("test:key").unwrap().unwrap();
        assert_eq!(retrieved.status, 200);
        assert_eq!(retrieved.body, br#"{"id":1}"#);
        assert_eq!(retrieved.etag, Some("\"abc123\"".into()));
    }

    #[test]
    fn test_entry_not_expired() {
        let entry = sample_entry(300);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_entry_expired() {
        let now = now_secs();
        let entry = CacheEntry {
            status: 200,
            headers: vec![],
            body: vec![],
            etag: None,
            cached_at: now - 600,
            expires_at: now - 1, // expired 1 second ago
        };
        assert!(entry.is_expired());
    }

    #[test]
    fn test_refresh_ttl() {
        let now = now_secs();
        let mut entry = CacheEntry {
            status: 200,
            headers: vec![],
            body: vec![],
            etag: None,
            cached_at: now - 600,
            expires_at: now - 1, // expired
        };
        assert!(entry.is_expired());

        entry.refresh_ttl(Duration::from_secs(300));
        assert!(!entry.is_expired());
        assert!(entry.expires_at > now);
    }

    #[test]
    fn test_remove() {
        let (store, _dir) = temp_cache();
        let entry = sample_entry(300);
        store.set("test:remove", &entry).unwrap();

        assert!(store.remove("test:remove").unwrap());
        assert!(store.get("test:remove").unwrap().is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let (store, _dir) = temp_cache();
        assert!(!store.remove("does_not_exist").unwrap());
    }

    #[test]
    fn test_remove_by_prefix() {
        let (store, _dir) = temp_cache();
        let entry = sample_entry(300);

        store.set("abc:GET:/repos/org/repo/issues", &entry).unwrap();
        store.set("abc:GET:/repos/org/repo/pulls", &entry).unwrap();
        store
            .set("abc:GET:/repos/other/repo/issues", &entry)
            .unwrap();
        store.set("xyz:GET:/repos/org/repo/issues", &entry).unwrap();

        let removed = store.remove_by_prefix("abc:GET:/repos/org/repo").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn test_count() {
        let (store, _dir) = temp_cache();
        assert_eq!(store.count().unwrap(), 0);

        let entry = sample_entry(300);
        store.set("key1", &entry).unwrap();
        store.set("key2", &entry).unwrap();
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn test_overwrite_entry() {
        let (store, _dir) = temp_cache();
        let entry1 = sample_entry(300);
        store.set("key", &entry1).unwrap();

        let mut entry2 = sample_entry(600);
        entry2.status = 304;
        store.set("key", &entry2).unwrap();

        let retrieved = store.get("key").unwrap().unwrap();
        assert_eq!(retrieved.status, 304);
    }
}
