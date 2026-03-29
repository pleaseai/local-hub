use std::path::Path;

use libsql::{Builder, Connection, Database};

use crate::error::Result;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS cache_entries (
    key        TEXT PRIMARY KEY,
    status     INTEGER NOT NULL,
    headers    TEXT NOT NULL,
    body       BLOB NOT NULL,
    etag       TEXT,
    cached_at  INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entity_deps (
    entity_id TEXT NOT NULL,
    cache_key TEXT NOT NULL,
    PRIMARY KEY (entity_id, cache_key)
);

CREATE INDEX IF NOT EXISTS idx_entity_deps_entity ON entity_deps(entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_deps_key ON entity_deps(cache_key);
"#;

/// Open or create a libsql database at the given path and run schema migrations.
pub async fn open_database(path: &Path) -> Result<Database> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Builder::new_local(path).build().await?;
    let conn = db.connect()?;
    conn.execute_batch(SCHEMA).await?;
    Ok(db)
}

/// Create a new connection from an existing database.
pub fn connect(db: &Database) -> Result<Connection> {
    Ok(db.connect()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_database_creates_tables() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = open_database(&db_path).await.unwrap();
        let conn = db.connect().unwrap();

        // Verify tables exist by querying them
        let mut rows = conn
            .query("SELECT count(*) FROM cache_entries", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 0);

        let mut rows = conn
            .query("SELECT count(*) FROM entity_deps", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 0);
    }

    #[tokio::test]
    async fn test_open_database_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested/deep/test.db");
        let _db = open_database(&db_path).await.unwrap();
        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn test_open_database_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let _db1 = open_database(&db_path).await.unwrap();
        // Opening again should not fail (IF NOT EXISTS)
        let _db2 = open_database(&db_path).await.unwrap();
    }
}
