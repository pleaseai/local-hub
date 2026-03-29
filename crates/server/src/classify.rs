use serde_json::Value;

/// Classification of an incoming request.
#[derive(Debug, PartialEq)]
pub enum RequestKind {
    /// REST GET request (cacheable).
    RestGet,
    /// REST mutation (POST/PUT/PATCH/DELETE, non-GraphQL).
    RestMutation,
    /// GraphQL query (cacheable).
    GraphqlQuery {
        query: String,
        variables: Option<String>,
    },
    /// GraphQL mutation (pass-through + invalidate).
    GraphqlMutation,
}

/// Classify an incoming request based on method, path, and optional body.
pub fn classify(method: &str, path: &str, body: Option<&[u8]>) -> RequestKind {
    if path == "/graphql" && method == "POST" {
        return classify_graphql(body);
    }

    if method == "GET" {
        RequestKind::RestGet
    } else {
        RequestKind::RestMutation
    }
}

fn classify_graphql(body: Option<&[u8]>) -> RequestKind {
    let Some(body) = body else {
        return RequestKind::GraphqlMutation; // No body = treat as mutation (safe default)
    };

    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return RequestKind::GraphqlMutation;
    };

    let Some(query_str) = value.get("query").and_then(|q| q.as_str()) else {
        return RequestKind::GraphqlMutation;
    };

    // Detect operation type from query text
    let trimmed = query_str.trim();
    if is_mutation(trimmed) {
        return RequestKind::GraphqlMutation;
    }

    // Extract variables as JSON string for cache key
    let variables = value
        .get("variables")
        .filter(|v| !v.is_null())
        .map(|v| v.to_string());

    RequestKind::GraphqlQuery {
        query: query_str.to_string(),
        variables,
    }
}

/// Check if a GraphQL operation text is a mutation or subscription (non-cacheable).
fn is_mutation(query: &str) -> bool {
    query.starts_with("mutation") || query.starts_with("subscription")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rest_get() {
        assert_eq!(
            classify("GET", "/repos/org/repo/issues", None),
            RequestKind::RestGet
        );
    }

    #[test]
    fn test_rest_post() {
        assert_eq!(
            classify("POST", "/repos/org/repo/issues", None),
            RequestKind::RestMutation
        );
    }

    #[test]
    fn test_rest_patch() {
        assert_eq!(
            classify("PATCH", "/repos/org/repo/issues/1", None),
            RequestKind::RestMutation
        );
    }

    #[test]
    fn test_rest_delete() {
        assert_eq!(
            classify("DELETE", "/repos/org/repo/issues/1", None),
            RequestKind::RestMutation
        );
    }

    #[test]
    fn test_graphql_query() {
        let body = br#"{"query": "query { viewer { login } }"}"#;
        let result = classify("POST", "/graphql", Some(body));
        match result {
            RequestKind::GraphqlQuery { query, variables } => {
                assert!(query.contains("viewer"));
                assert!(variables.is_none());
            }
            _ => panic!("expected GraphqlQuery"),
        }
    }

    #[test]
    fn test_graphql_query_unnamed() {
        // Unnamed query (starts with '{')
        let body = br#"{"query": "{ viewer { login } }"}"#;
        let result = classify("POST", "/graphql", Some(body));
        match result {
            RequestKind::GraphqlQuery { .. } => {}
            _ => panic!("expected GraphqlQuery for unnamed query"),
        }
    }

    #[test]
    fn test_graphql_mutation() {
        let body = br#"{"query": "mutation { createIssue(input: {}) { issue { id } } }"}"#;
        assert_eq!(
            classify("POST", "/graphql", Some(body)),
            RequestKind::GraphqlMutation
        );
    }

    #[test]
    fn test_graphql_with_variables() {
        let body = br#"{"query": "query($owner: String!) { repository(owner: $owner) { id } }", "variables": {"owner": "octocat"}}"#;
        let result = classify("POST", "/graphql", Some(body));
        match result {
            RequestKind::GraphqlQuery { variables, .. } => {
                assert!(variables.is_some());
                assert!(variables.unwrap().contains("octocat"));
            }
            _ => panic!("expected GraphqlQuery"),
        }
    }

    #[test]
    fn test_graphql_no_body() {
        assert_eq!(
            classify("POST", "/graphql", None),
            RequestKind::GraphqlMutation
        );
    }

    #[test]
    fn test_graphql_malformed_body() {
        assert_eq!(
            classify("POST", "/graphql", Some(b"not json")),
            RequestKind::GraphqlMutation
        );
    }

    #[test]
    fn test_non_graphql_post() {
        // POST to non-graphql path
        assert_eq!(
            classify("POST", "/repos/org/repo/issues", Some(b"{}")),
            RequestKind::RestMutation
        );
    }
}
