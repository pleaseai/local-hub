use sha2::{Digest, Sha256};

/// Generate a cache key from token, method, URL path, and query string.
///
/// Format: `{token_hash}:{method}:{path}?{query}`
/// Token hash is the first 16 hex chars of SHA-256(token).
pub fn cache_key(token: &str, method: &str, path: &str, query: Option<&str>) -> String {
    let token_hash = token_hash(token);
    match query {
        Some(q) if !q.is_empty() => format!("{token_hash}:{method}:{path}?{q}"),
        _ => format!("{token_hash}:{method}:{path}"),
    }
}

/// Compute a truncated SHA-256 hash of the token (first 16 hex chars).
/// Tokens are never stored — only this hash is used as a namespace prefix.
fn token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // 8 bytes = 16 hex chars
}

/// Extract the token hash prefix from a cache key.
#[allow(dead_code)]
pub fn extract_token_prefix(key: &str) -> Option<&str> {
    key.split(':').next()
}

/// Generate an invalidation prefix for a given token and path pattern.
/// Used to bulk-invalidate cache entries related to a resource.
pub fn invalidation_prefix(token: &str, path_prefix: &str) -> String {
    let token_hash = token_hash(token);
    format!("{token_hash}:GET:{path_prefix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_without_query() {
        let key = cache_key("ghp_test123", "GET", "/repos/org/repo/issues", None);
        // Should start with 16 hex chars token hash
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        assert_eq!(parts[0].len(), 16);
        assert!(parts[1].starts_with("GET:/repos/org/repo/issues"));
        assert!(!key.contains('?'));
    }

    #[test]
    fn test_cache_key_with_query() {
        let key = cache_key(
            "ghp_test123",
            "GET",
            "/repos/org/repo/issues",
            Some("state=open&per_page=30"),
        );
        assert!(key.contains("?state=open&per_page=30"));
    }

    #[test]
    fn test_cache_key_with_empty_query() {
        let key = cache_key("ghp_test123", "GET", "/repos/org/repo", Some(""));
        assert!(!key.contains('?'));
    }

    #[test]
    fn test_same_token_same_hash() {
        let key1 = cache_key("ghp_abc", "GET", "/repos/a", None);
        let key2 = cache_key("ghp_abc", "GET", "/repos/b", None);
        let prefix1 = extract_token_prefix(&key1).unwrap();
        let prefix2 = extract_token_prefix(&key2).unwrap();
        assert_eq!(prefix1, prefix2);
    }

    #[test]
    fn test_different_tokens_different_hash() {
        let key1 = cache_key("ghp_token_a", "GET", "/repos/a", None);
        let key2 = cache_key("ghp_token_b", "GET", "/repos/a", None);
        let prefix1 = extract_token_prefix(&key1).unwrap();
        let prefix2 = extract_token_prefix(&key2).unwrap();
        assert_ne!(prefix1, prefix2);
    }

    #[test]
    fn test_extract_token_prefix() {
        let key = cache_key("ghp_test", "GET", "/repos/a", None);
        let prefix = extract_token_prefix(&key).unwrap();
        assert_eq!(prefix.len(), 16);
    }

    #[test]
    fn test_invalidation_prefix() {
        let prefix = invalidation_prefix("ghp_test", "/repos/org/repo");
        assert!(prefix.ends_with("GET:/repos/org/repo"));
        assert_eq!(prefix.split(':').next().unwrap().len(), 16);
    }
}
