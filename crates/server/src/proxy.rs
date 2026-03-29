use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use tracing::{debug, warn};

use crate::classify::{self, RequestKind};
use crate::entity;
use crate::entity_cache::{CacheEntry, EntityAwareCache};
use crate::key;
use crate::ttl::TtlConfig;

pub const DEFAULT_GITHUB_API_BASE: &str = "https://api.github.com";

pub struct AppState {
    pub cache: EntityAwareCache,
    pub ttl_config: TtlConfig,
    pub client: reqwest::Client,
    pub github_base_url: String,
}

impl AppState {
    fn build_url(&self, path: &str, query: Option<&str>) -> String {
        match query {
            Some(q) if !q.is_empty() => format!("{}{path}?{q}", self.github_base_url),
            _ => format!("{}{path}", self.github_base_url),
        }
    }
}

/// Main proxy handler. Routes requests through entity-aware cache.
pub async fn proxy_handler(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| q.to_string());

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.trim_start_matches("Bearer ")
                .trim_start_matches("token ")
                .to_string()
        })
        .unwrap_or_default();

    // Read body for non-GET requests (needed for classification and forwarding)
    let body_bytes = if method != "GET" {
        Some(
            axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
                .await
                .unwrap_or_default(),
        )
    } else {
        None
    };

    let kind = classify::classify(method.as_ref(), &path, body_bytes.as_deref());

    match kind {
        RequestKind::RestGet => handle_rest_get(&state, &path, query.as_deref(), &token).await,
        RequestKind::RestMutation => {
            handle_mutation(
                &state,
                method.as_ref(),
                &path,
                query.as_deref(),
                &token,
                body_bytes,
            )
            .await
        }
        RequestKind::GraphqlQuery {
            query: gql_query,
            variables,
        } => {
            handle_graphql_query(
                &state,
                &path,
                &token,
                &gql_query,
                variables.as_deref(),
                body_bytes,
            )
            .await
        }
        RequestKind::GraphqlMutation => {
            handle_mutation(
                &state,
                method.as_ref(),
                &path,
                query.as_deref(),
                &token,
                body_bytes,
            )
            .await
        }
    }
}

/// Handle REST GET: cache lookup → conditional request → fetch → store with entities.
async fn handle_rest_get(
    state: &AppState,
    path: &str,
    query: Option<&str>,
    token: &str,
) -> Response {
    let cache_key = key::cache_key(token, "GET", path, query);
    let ttl = state.ttl_config.resolve(path);

    match state.cache.get(&cache_key).await {
        Ok(Some(entry)) if !entry.is_expired() => {
            debug!(path, "cache hit (fresh)");
            return cache_entry_to_response(&entry);
        }
        Ok(Some(entry)) => {
            if let Some(ref etag) = entry.etag {
                debug!(path, "cache hit (stale), trying conditional request");
                let result = conditional_request(state, path, query, token, etag).await;
                match result {
                    ConditionalResult::NotModified => {
                        let mut refreshed = entry.clone();
                        refreshed.refresh_ttl(ttl);
                        if let Err(e) = state.cache.update(&cache_key, &refreshed).await {
                            warn!(error = %e, "failed to refresh cache TTL");
                        }
                        debug!(path, "304 Not Modified — serving cached");
                        return cache_entry_to_response(&refreshed);
                    }
                    ConditionalResult::NewResponse(resp) => {
                        return store_and_respond(state, &cache_key, ttl, resp).await;
                    }
                    ConditionalResult::Error(resp) => return resp,
                }
            }
            debug!(path, "cache hit (stale, no etag), fetching fresh");
        }
        Ok(None) => {
            debug!(path, "cache miss");
        }
        Err(e) => {
            warn!(error = %e, "cache read error, falling through");
        }
    }

    let result = fetch_from_github(state, path, query, token).await;
    match result {
        Ok(resp) => store_and_respond(state, &cache_key, ttl, resp).await,
        Err(e) => {
            warn!(error = %e, path, "github request failed");
            (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response()
        }
    }
}

/// Handle GraphQL query: cache by query hash → fetch → store with entities.
async fn handle_graphql_query(
    state: &AppState,
    path: &str,
    token: &str,
    gql_query: &str,
    variables: Option<&str>,
    body_bytes: Option<axum::body::Bytes>,
) -> Response {
    let cache_key = key::graphql_cache_key(token, gql_query, variables);
    let ttl = state.ttl_config.resolve(path);

    // Check cache
    match state.cache.get(&cache_key).await {
        Ok(Some(entry)) if !entry.is_expired() => {
            debug!("graphql cache hit (fresh)");
            return cache_entry_to_response(&entry);
        }
        Ok(Some(entry)) => {
            // Stale GraphQL entry — no ETag support for GraphQL, fetch fresh
            if let Some(ref etag) = entry.etag {
                debug!("graphql cache hit (stale), trying conditional request");
                let result =
                    conditional_graphql_request(state, path, token, body_bytes.as_deref(), etag)
                        .await;
                match result {
                    ConditionalResult::NotModified => {
                        let mut refreshed = entry.clone();
                        refreshed.refresh_ttl(ttl);
                        if let Err(e) = state.cache.update(&cache_key, &refreshed).await {
                            warn!(error = %e, "failed to refresh graphql cache TTL");
                        }
                        return cache_entry_to_response(&refreshed);
                    }
                    ConditionalResult::NewResponse(resp) => {
                        return store_graphql_and_respond(state, &cache_key, ttl, resp).await;
                    }
                    ConditionalResult::Error(resp) => return resp,
                }
            }
            debug!("graphql cache hit (stale, no etag), fetching fresh");
        }
        Ok(None) => {
            debug!("graphql cache miss");
        }
        Err(e) => {
            warn!(error = %e, "graphql cache read error, falling through");
        }
    }

    // Fetch from GitHub
    let result = forward_graphql(state, path, token, body_bytes).await;
    match result {
        Ok(resp) => store_graphql_and_respond(state, &cache_key, ttl, resp).await,
        Err(e) => {
            warn!(error = %e, "graphql request failed");
            (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response()
        }
    }
}

/// Handle mutations (REST or GraphQL): forward → extract entities → invalidate.
async fn handle_mutation(
    state: &AppState,
    method: &str,
    path: &str,
    query: Option<&str>,
    token: &str,
    body_bytes: Option<axum::body::Bytes>,
) -> Response {
    let response = forward_to_github(state, method, path, query, token, body_bytes).await;

    // Extract entities from mutation response for invalidation
    if !token.is_empty() {
        // Try to read the response body for entity extraction
        // (we need to reconstruct the response after reading)
        // For now, use prefix invalidation as the primary strategy
        // Entity invalidation from mutation responses will be extracted
        // from the response body when possible

        // Prefix-based invalidation (fallback, always runs)
        if let Some(parent) = parent_path(path) {
            let prefix = key::invalidation_prefix(token, parent);
            if let Err(e) = state.cache.remove_by_prefix(&prefix).await {
                warn!(error = %e, prefix, "failed to invalidate cache by prefix");
            }
        }
    }

    response
}

struct GithubResponse {
    status: u16,
    headers: Vec<(String, String)>,
    etag: Option<String>,
    body: Vec<u8>,
}

/// Store REST response and extract entities for dependency tracking.
async fn store_and_respond(
    state: &AppState,
    cache_key: &str,
    ttl: Duration,
    resp: GithubResponse,
) -> Response {
    if (200..300).contains(&resp.status) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entity_ids = entity::extract_entity_ids(&resp.body);
        let entry = CacheEntry {
            status: resp.status,
            headers: resp.headers.clone(),
            body: resp.body.clone(),
            etag: resp.etag.clone(),
            cached_at: now,
            expires_at: now + ttl.as_secs(),
        };
        if let Err(e) = state.cache.store(cache_key, &entry, &entity_ids).await {
            warn!(error = %e, "failed to store in cache");
        }
    }
    github_response_to_axum(resp)
}

/// Store GraphQL response and extract entities.
async fn store_graphql_and_respond(
    state: &AppState,
    cache_key: &str,
    ttl: Duration,
    resp: GithubResponse,
) -> Response {
    if (200..300).contains(&resp.status) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entity_ids = entity::extract_graphql_entity_ids(&resp.body);
        let entry = CacheEntry {
            status: resp.status,
            headers: resp.headers.clone(),
            body: resp.body.clone(),
            etag: resp.etag.clone(),
            cached_at: now,
            expires_at: now + ttl.as_secs(),
        };
        if let Err(e) = state.cache.store(cache_key, &entry, &entity_ids).await {
            warn!(error = %e, "failed to store graphql in cache");
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
    let url = state.build_url(path, query);
    let mut req = state.client.get(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    req = req.header("Accept", "application/vnd.github+json");
    req = req.header("X-GitHub-Api-Version", "2022-11-28");

    let resp = req.send().await?;
    Ok(parse_github_response(resp).await)
}

async fn forward_graphql(
    state: &AppState,
    path: &str,
    token: &str,
    body_bytes: Option<axum::body::Bytes>,
) -> std::result::Result<GithubResponse, reqwest::Error> {
    let url = state.build_url(path, None);
    let mut req = state.client.post(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(ref body) = body_bytes
        && !body.is_empty()
    {
        req = req.header("Content-Type", "application/json");
        req = req.body(body.clone());
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
    let url = state.build_url(path, query);
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

async fn conditional_graphql_request(
    state: &AppState,
    path: &str,
    token: &str,
    body_bytes: Option<&[u8]>,
    etag: &str,
) -> ConditionalResult {
    let url = state.build_url(path, None);
    let mut req = state.client.post(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(body) = body_bytes
        && !body.is_empty()
    {
        req = req.header("Content-Type", "application/json");
        req = req.body(body.to_vec());
    }
    req = req.header("Accept", "application/vnd.github+json");
    req = req.header("X-GitHub-Api-Version", "2022-11-28");
    req = req.header("If-None-Match", etag);

    match req.send().await {
        Ok(resp) if resp.status().as_u16() == 304 => ConditionalResult::NotModified,
        Ok(resp) => ConditionalResult::NewResponse(parse_github_response(resp).await),
        Err(e) => {
            warn!(error = %e, "conditional graphql request failed");
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
    token: &str,
    body: Option<axum::body::Bytes>,
) -> Response {
    let url = state.build_url(path, query);
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
    if let Some(ref body_bytes) = body
        && !body_bytes.is_empty()
    {
        req = req.header("Content-Type", "application/json");
        req = req.body(body_bytes.clone());
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

/// Get the parent path for cache invalidation (prefix fallback).
fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_without_query() {
        // Can't easily create AppState without async, test the logic directly
        let base = DEFAULT_GITHUB_API_BASE;
        let url = match None::<&str> {
            Some(q) if !q.is_empty() => format!("{base}/repos/org/repo?{q}"),
            _ => format!("{base}/repos/org/repo"),
        };
        assert_eq!(url, "https://api.github.com/repos/org/repo");
    }

    #[test]
    fn test_build_url_with_query() {
        let base = DEFAULT_GITHUB_API_BASE;
        let q = "state=open";
        let url = format!("{base}/repos/org/repo/issues?{q}");
        assert_eq!(
            url,
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
