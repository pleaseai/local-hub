use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::service::TowerToHyperService;
use tokio::net::{TcpListener, UnixListener};
use tower::Service;
use tracing::info;

use crate::proxy::{self, AppState};

/// Build the axum router with the proxy handler as a fallback.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .fallback(proxy::proxy_handler)
        .with_state(state)
}

/// Start the TCP listener on the given port.
pub async fn serve_tcp(router: Router, port: u16) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP listener on http://{addr}");
    axum::serve(listener, router).await
}

/// Start the Unix socket listener at the given path.
pub async fn serve_unix(router: Router, socket_path: PathBuf) -> std::io::Result<()> {
    // Remove stale socket file
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    info!(path = %socket_path.display(), "Unix socket listener");

    let mut make_service = router.into_make_service();
    loop {
        let (stream, _addr) = listener.accept().await?;
        let tower_svc = unwrap_infallible(make_service.call(()).await);
        let hyper_svc = TowerToHyperService::new(tower_svc);
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            if let Err(e) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection(io, hyper_svc)
                .await
            {
                tracing::warn!(error = %e, "unix socket connection error");
            }
        });
    }
}

fn unwrap_infallible<T>(result: Result<T, std::convert::Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}
