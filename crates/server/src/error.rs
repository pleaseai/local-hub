use std::io;

#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum Error {
    #[error("cache error: {0}")]
    Cache(#[from] redb::Error),

    #[error("cache database error: {0}")]
    CacheDatabase(#[from] redb::DatabaseError),

    #[error("cache table error: {0}")]
    CacheTable(#[from] redb::TableError),

    #[error("cache storage error: {0}")]
    CacheStorage(#[from] redb::StorageError),

    #[error("cache commit error: {0}")]
    CacheCommit(#[from] redb::CommitError),

    #[error("cache transaction error: {0}")]
    CacheTransaction(#[from] redb::TransactionError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[allow(clippy::result_large_err)]
pub type Result<T> = std::result::Result<T, Error>;
