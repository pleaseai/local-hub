//! Integration tests using emulate.dev as a GitHub API mock server.
//!
//! These tests require `npx emulate` to be available.
//! Run with: `cargo test --test integration_test`
//!
//! Each test starts emulate.dev on a unique port, creates a local-hub proxy
//! with axum's `oneshot`, and verifies caching behavior end-to-end.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use http::Request;
use local_hub::{CacheStore, TtlConfig, build_router, proxy};
use tower::ServiceExt;

/// Start emulate.dev GitHub service on a specific port.
/// Returns None if npx/emulate is not available.
fn start_emulate(port: u16) -> Option<Child> {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/emulate.config.yaml");

    let child = Command::new("npx")
        .args([
            "-y",
            "emulate",
            "--service",
            "github",
            "--port",
            &port.to_string(),
            "--seed",
            config_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // Wait for emulate to be ready
    for _ in 0..50 {
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return Some(child);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    eprintln!("emulate.dev failed to start on port {port}");
    Some(child)
}

/// Create an AppState pointing to the emulate.dev server.
fn create_test_state(emulate_port: u16) -> (Arc<proxy::AppState>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test-cache.redb");

    let state = Arc::new(proxy::AppState {
        cache: CacheStore::open(&db_path).unwrap(),
        ttl_config: TtlConfig::new(Duration::from_secs(300)),
        client: reqwest::Client::new(),
        github_base_url: format!("http://127.0.0.1:{emulate_port}"),
    });
    (state, dir)
}

#[tokio::test]
async fn test_cache_miss_then_hit() {
    let emulate_port = 14001;
    let mut emulate = match start_emulate(emulate_port) {
        Some(child) => child,
        None => {
            eprintln!("SKIP: npx emulate not available");
            return;
        }
    };

    let (state, _dir) = create_test_state(emulate_port);
    let app = build_router(state);

    // First request — cache miss
    let req = Request::builder()
        .uri("/repos/testorg/test-repo")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "miss");

    // Second request — cache hit
    let req = Request::builder()
        .uri("/repos/testorg/test-repo")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "hit");

    emulate.kill().ok();
}

#[tokio::test]
async fn test_token_isolation() {
    let emulate_port = 14002;
    let mut emulate = match start_emulate(emulate_port) {
        Some(child) => child,
        None => {
            eprintln!("SKIP: npx emulate not available");
            return;
        }
    };

    let (state, _dir) = create_test_state(emulate_port);
    let app = build_router(state);

    // Request with token A — cache miss
    let req = Request::builder()
        .uri("/repos/testorg/test-repo")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "miss");

    // Request with token B — should also be cache miss (different namespace)
    let req = Request::builder()
        .uri("/repos/testorg/test-repo")
        .header("Authorization", "Bearer gho_test_token_user")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "miss");

    // Token A again — should be cache hit
    let req = Request::builder()
        .uri("/repos/testorg/test-repo")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "hit");

    emulate.kill().ok();
}

#[tokio::test]
async fn test_write_invalidation() {
    let emulate_port = 14003;
    let mut emulate = match start_emulate(emulate_port) {
        Some(child) => child,
        None => {
            eprintln!("SKIP: npx emulate not available");
            return;
        }
    };

    let (state, _dir) = create_test_state(emulate_port);
    let app = build_router(state);

    // GET issues — cache miss
    let req = Request::builder()
        .uri("/repos/testorg/test-repo/issues")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "miss");

    // GET issues — cache hit
    let req = Request::builder()
        .uri("/repos/testorg/test-repo/issues")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "hit");

    // POST new issue — invalidates issues cache
    let req = Request::builder()
        .method("POST")
        .uri("/repos/testorg/test-repo/issues")
        .header("Authorization", "Bearer gho_test_token_admin")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"title":"test issue","body":"created by integration test"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    assert!(status == 201 || status == 200, "POST status: {status}");

    // GET issues — cache miss (invalidated by POST)
    let req = Request::builder()
        .uri("/repos/testorg/test-repo/issues")
        .header("Authorization", "Bearer gho_test_token_admin")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers().get("x-local-hub-cache").unwrap(), "miss");

    emulate.kill().ok();
}
