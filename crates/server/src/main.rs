use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use local_hub::{CacheStore, TtlConfig, proxy, server};
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_PORT: u16 = 8787;
const DEFAULT_SOCKET: &str = ".local-hub/local-hub.sock";
const DEFAULT_CACHE_DIR: &str = ".local-hub/cache.redb";
const DEFAULT_TTL: u64 = 300;

#[derive(Parser)]
#[command(name = "local-hub", version, about = "Local GitHub API cache proxy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy server
    Start {
        /// TCP port to listen on
        #[arg(short, long, default_value_t = DEFAULT_PORT, env = "LOCAL_HUB_PORT")]
        port: u16,

        /// Unix socket path
        #[arg(short, long, env = "LOCAL_HUB_SOCKET")]
        socket: Option<PathBuf>,

        /// Cache database path
        #[arg(short, long, env = "LOCAL_HUB_CACHE_DIR")]
        cache_dir: Option<PathBuf>,

        /// Default TTL in seconds
        #[arg(short, long, default_value_t = DEFAULT_TTL, env = "LOCAL_HUB_TTL")]
        ttl: u64,

        /// GitHub API base URL (for testing with emulate.dev)
        #[arg(long, env = "LOCAL_HUB_GITHUB_URL")]
        github_url: Option<String>,
    },
    /// Show cache statistics
    Status {
        /// Cache database path
        #[arg(short, long, env = "LOCAL_HUB_CACHE_DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Flush cache entries
    Flush {
        /// Only flush entries matching this prefix
        pattern: Option<String>,

        /// Cache database path
        #[arg(short, long, env = "LOCAL_HUB_CACHE_DIR")]
        cache_dir: Option<PathBuf>,
    },
}

fn resolve_home_path(relative: &str) -> PathBuf {
    dirs_next().join(relative)
}

fn dirs_next() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }
    #[cfg(not(unix))]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("local_hub=info,warn")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            port,
            socket,
            cache_dir,
            ttl,
            github_url,
        } => {
            let cache_path = cache_dir.unwrap_or_else(|| resolve_home_path(DEFAULT_CACHE_DIR));
            let socket_path = socket.unwrap_or_else(|| resolve_home_path(DEFAULT_SOCKET));

            let cache = CacheStore::open(&cache_path)?;
            let ttl_config = TtlConfig::new(std::time::Duration::from_secs(ttl));

            let client = reqwest::Client::builder()
                .user_agent("local-hub/0.1")
                .build()?;

            let github_base_url =
                github_url.unwrap_or_else(|| proxy::DEFAULT_GITHUB_API_BASE.to_string());

            let state = Arc::new(proxy::AppState {
                cache,
                ttl_config,
                client,
                github_base_url,
            });

            info!(port, socket = %socket_path.display(), cache = %cache_path.display(), "starting local-hub");

            let router = server::build_router(state);
            let tcp_router = router.clone();
            let unix_router = router;

            tokio::try_join!(
                server::serve_tcp(tcp_router, port),
                server::serve_unix(unix_router, socket_path),
            )?;
        }
        Commands::Status { cache_dir } => {
            let cache_path = cache_dir.unwrap_or_else(|| resolve_home_path(DEFAULT_CACHE_DIR));
            if !cache_path.exists() {
                println!("No cache database found at {}", cache_path.display());
                return Ok(());
            }
            let cache = CacheStore::open(&cache_path)?;
            let count = cache.count()?;
            println!("Cache: {}", cache_path.display());
            println!("Entries: {count}");
        }
        Commands::Flush { pattern, cache_dir } => {
            let cache_path = cache_dir.unwrap_or_else(|| resolve_home_path(DEFAULT_CACHE_DIR));
            if !cache_path.exists() {
                println!("No cache database found at {}", cache_path.display());
                return Ok(());
            }
            let cache = CacheStore::open(&cache_path)?;
            match pattern {
                Some(prefix) => {
                    let removed = cache.remove_by_prefix(&prefix)?;
                    println!("Removed {removed} entries matching '{prefix}'");
                }
                None => {
                    let count = cache.count()?;
                    let removed = cache.remove_by_prefix("")?;
                    println!("Flushed {removed} entries (was {count})");
                }
            }
        }
    }

    Ok(())
}
