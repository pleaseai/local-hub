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
pub(crate) fn token_hash(token: &str) -> String {
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

/// Generate a cache key for a GraphQL query.
/// Format: `{token_hash}:GQL:{sha256(query + variables_json)}`
pub fn graphql_cache_key(token: &str, query: &str, variables: Option<&str>) -> String {
    let token_hash = token_hash(token);
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    if let Some(vars) = variables {
        hasher.update(b"|");
        hasher.update(vars.as_bytes());
    }
    let result = hasher.finalize();
    let query_hash = hex::encode(&result[..16]); // 32 hex chars for uniqueness
    format!("{token_hash}:GQL:{query_hash}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_without_query() {
        let key = cache_key("ghp_test123", "GET", "/repos/org/repo/issues", None);
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

    #[test]
    fn test_graphql_cache_key_deterministic() {
        let key1 = graphql_cache_key("ghp_test", "query { viewer { login } }", None);
        let key2 = graphql_cache_key("ghp_test", "query { viewer { login } }", None);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_graphql_cache_key_different_queries() {
        let key1 = graphql_cache_key("ghp_test", "query { viewer { login } }", None);
        let key2 = graphql_cache_key("ghp_test", "query { viewer { name } }", None);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_graphql_cache_key_different_variables() {
        let key1 = graphql_cache_key("ghp_test", "query($n:Int!){}", Some(r#"{"n":1}"#));
        let key2 = graphql_cache_key("ghp_test", "query($n:Int!){}", Some(r#"{"n":2}"#));
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_graphql_cache_key_with_vs_without_variables() {
        let key1 = graphql_cache_key("ghp_test", "query { viewer }", None);
        let key2 = graphql_cache_key("ghp_test", "query { viewer }", Some("{}"));
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_graphql_cache_key_format() {
        let key = graphql_cache_key("ghp_test", "query { viewer }", None);
        assert!(key.contains(":GQL:"));
        let parts: Vec<&str> = key.splitn(3, ':').collect();
        assert_eq!(parts[0].len(), 16); // token hash
        assert_eq!(parts[1], "GQL");
        assert_eq!(parts[2].len(), 32); // query hash
    }
}
