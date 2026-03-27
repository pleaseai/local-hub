#![allow(clippy::result_large_err)]

pub mod cache;
pub mod error;
pub mod key;
pub mod proxy;
pub mod server;
pub mod ttl;

pub use cache::CacheStore;
pub use proxy::AppState;
pub use server::build_router;
pub use ttl::TtlConfig;
