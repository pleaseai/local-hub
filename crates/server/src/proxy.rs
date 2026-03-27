use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use tracing::{debug, warn};

use crate::cache::{CacheEntry, CacheStore};
use crate::key;
use crate::ttl::TtlConfig;

const GITHUB_API_BASE: &str = "https://api.github.com";

pub struct AppState {
    pub cache: CacheStore,
    pub ttl_config: TtlConfig,
    pub client: reqwest::Client,
}

/// Main proxy handler. Intercepts all requests and applies caching logic.
pub async fn proxy_handler(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| q.to_string());

    // Extract token from Authorization header
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("Bearer ").trim_start_matches("token "))
        .unwrap_or("");

    // Only cache GET requests
    if method != "GET" {
        let response = forward_to_github(
            &state,
            method.as_ref(),
            &path,
            query.as_deref(),
            req.headers(),
            token,
        )
        .await;
        // Invalidate related cache on mutations
        if !token.is_empty() {
            if let Some(parent) = parent_path(&path) {
                let prefix = key::invalidation_prefix(token, parent);
                if let Err(e) = state.cache.remove_by_prefix(&prefix) {
                    warn!(error = %e, prefix, "failed to invalidate cache");
                }
            }
        }
        return response;
    }

    let cache_key = key::cache_key(token, "GET", &path, query.as_deref());
    let ttl = state.ttl_config.resolve(&path);

    // Check cache
    match state.cache.get(&cache_key) {
        Ok(Some(entry)) if !entry.is_expired() => {
            debug!(path, "cache hit (fresh)");
            return cache_entry_to_response(&entry);
        }
        Ok(Some(entry)) => {
            // Expired but has ETag — try conditional request
            if let Some(ref etag) = entry.etag {
                debug!(path, "cache hit (stale), trying conditional request");
                let response =
                    conditional_request(&state, &path, query.as_deref(), token, etag).await;
                match response {
                    ConditionalResult::NotModified => {
                        let mut refreshed = entry.clone();
                        refreshed.refresh_ttl(ttl);
                        if let Err(e) = state.cache.set(&cache_key, &refreshed) {
                            warn!(error = %e, "failed to refresh cache TTL");
                        }
                        debug!(path, "304 Not Modified — serving cached");
                        return cache_entry_to_response(&refreshed);
                    }
                    ConditionalResult::NewResponse(resp) => {
                        return store_and_respond(&state, &cache_key, ttl, resp);
                    }
                    ConditionalResult::Error(resp) => return resp,
                }
            }
            // No ETag — fall through to fresh request
            debug!(path, "cache hit (stale, no etag), fetching fresh");
        }
        Ok(None) => {
            debug!(path, "cache miss");
        }
        Err(e) => {
            warn!(error = %e, "cache read error, falling through");
        }
    }

    // Fetch from GitHub
    let result = fetch_from_github(&state, &path, query.as_deref(), token).await;
    match result {
        Ok(resp) => store_and_respond(&state, &cache_key, ttl, resp),
        Err(e) => {
            warn!(error = %e, path, "github request failed");
            (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response()
        }
    }
}

struct GithubResponse {
    status: u16,
    headers: Vec<(String, String)>,
    etag: Option<String>,
    body: Vec<u8>,
}

fn store_and_respond(
    state: &AppState,
    cache_key: &str,
    ttl: Duration,
    resp: GithubResponse,
) -> Response {
    // Only cache successful responses
    if (200..300).contains(&resp.status) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = CacheEntry {
            status: resp.status,
            headers: resp.headers.clone(),
            body: resp.body.clone(),
            etag: resp.etag.clone(),
            cached_at: now,
            expires_at: now + ttl.as_secs(),
        };
        if let Err(e) = state.cache.set(cache_key, &entry) {
            warn!(error = %e, "failed to store in cache");
        }
    }
    github_response_to_axum(resp)
}

fn cache_entry_to_response(entry: &CacheEntry) -> Response {
    let mut builder = Response::builder().status(entry.status);
    for (name, value) in &entry.headers {
        if let Ok(v) = HeaderValue::from_str(value) {
            builder = builder.header(name.as_str(), v);
        }
    }
    builder
        .header("x-local-hub-cache", "hit")
        .body(Body::from(entry.body.clone()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn github_response_to_axum(resp: GithubResponse) -> Response {
    let mut builder = Response::builder().status(resp.status);
    for (name, value) in &resp.headers {
        if let Ok(v) = HeaderValue::from_str(value) {
            builder = builder.header(name.as_str(), v);
        }
    }
    builder
        .header("x-local-hub-cache", "miss")
        .body(Body::from(resp.body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn fetch_from_github(
    state: &AppState,
    path: &str,
    query: Option<&str>,
    token: &str,
) -> std::result::Result<GithubResponse, reqwest::Error> {
    let url = build_github_url(path, query);
    let mut req = state.client.get(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    req = req.header("Accept", "application/vnd.github+json");
    req = req.header("X-GitHub-Api-Version", "2022-11-28");

    let resp = req.send().await?;
    Ok(parse_github_response(resp).await)
}

enum ConditionalResult {
    NotModified,
    NewResponse(GithubResponse),
    Error(Response),
}

async fn conditional_request(
    state: &AppState,
    path: &str,
    query: Option<&str>,
    token: &str,
    etag: &str,
) -> ConditionalResult {
    let url = build_github_url(path, query);
    let mut req = state.client.get(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    req = req.header("Accept", "application/vnd.github+json");
    req = req.header("X-GitHub-Api-Version", "2022-11-28");
    req = req.header("If-None-Match", etag);

    match req.send().await {
        Ok(resp) if resp.status().as_u16() == 304 => ConditionalResult::NotModified,
        Ok(resp) => ConditionalResult::NewResponse(parse_github_response(resp).await),
        Err(e) => {
            warn!(error = %e, "conditional request failed");
            ConditionalResult::Error(
                (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response(),
            )
        }
    }
}

async fn forward_to_github(
    state: &AppState,
    method: &str,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    token: &str,
) -> Response {
    let url = build_github_url(path, query);
    let req_builder = match method {
        "POST" => state.client.post(&url),
        "PUT" => state.client.put(&url),
        "PATCH" => state.client.patch(&url),
        "DELETE" => state.client.delete(&url),
        _ => state.client.get(&url),
    };

    let mut req = req_builder;
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    // Forward content-type if present
    if let Some(ct) = headers.get("content-type") {
        if let Ok(v) = ct.to_str() {
            req = req.header("Content-Type", v);
        }
    }
    req = req.header("Accept", "application/vnd.github+json");
    req = req.header("X-GitHub-Api-Version", "2022-11-28");

    match req.send().await {
        Ok(resp) => {
            let gh_resp = parse_github_response(resp).await;
            github_response_to_axum(gh_resp)
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response(),
    }
}

async fn parse_github_response(resp: reqwest::Response) -> GithubResponse {
    let status = resp.status().as_u16();
    let etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .filter(|(name, _)| {
            let n = name.as_str();
            // Forward relevant headers
            n.starts_with("x-ratelimit")
                || n == "content-type"
                || n == "etag"
                || n == "link"
                || n == "x-github-media-type"
        })
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect();

    let body = resp.bytes().await.unwrap_or_default().to_vec();

    GithubResponse {
        status,
        headers,
        etag,
        body,
    }
}

fn build_github_url(path: &str, query: Option<&str>) -> String {
    match query {
        Some(q) if !q.is_empty() => format!("{GITHUB_API_BASE}{path}?{q}"),
        _ => format!("{GITHUB_API_BASE}{path}"),
    }
}

/// Get the parent path for cache invalidation.
/// e.g., "/repos/org/repo/issues/1" → "/repos/org/repo/issues"
fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_github_url_without_query() {
        assert_eq!(
            build_github_url("/repos/org/repo", None),
            "https://api.github.com/repos/org/repo"
        );
    }

    #[test]
    fn test_build_github_url_with_query() {
        assert_eq!(
            build_github_url("/repos/org/repo/issues", Some("state=open")),
            "https://api.github.com/repos/org/repo/issues?state=open"
        );
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(
            parent_path("/repos/org/repo/issues/1"),
            Some("/repos/org/repo/issues")
        );
        assert_eq!(parent_path("/repos/org/repo"), Some("/repos/org"));
        assert_eq!(parent_path("/repos"), Some(""));
    }

    #[test]
    fn test_parent_path_root() {
        assert_eq!(parent_path("/"), Some(""));
    }
}
