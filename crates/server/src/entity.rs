use serde_json::Value;

/// Extract GitHub `node_id` values from a REST JSON response body.
/// Handles single objects and arrays. Top-level extraction only.
pub fn extract_entity_ids(body: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };

    match value {
        Value::Object(ref obj) => extract_node_id_from_object(obj),
        Value::Array(ref arr) => arr
            .iter()
            .filter_map(|v| v.as_object())
            .flat_map(extract_node_id_from_object)
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract entity IDs from a GraphQL response body.
/// Looks for `id` fields in the top-level `data` object values.
pub fn extract_graphql_entity_ids(body: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };

    let Some(data) = value.get("data").and_then(|d| d.as_object()) else {
        return Vec::new();
    };

    let mut ids = Vec::new();
    for (_key, val) in data {
        collect_graphql_ids(val, &mut ids);
    }
    ids
}

fn extract_node_id_from_object(obj: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(Value::String(node_id)) = obj.get("node_id")
        && !node_id.is_empty()
    {
        ids.push(node_id.clone());
    }
    ids
}

/// Collect `id` fields from a GraphQL value (top-level traversal of objects).
fn collect_graphql_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(id)) = obj.get("id")
                && !id.is_empty()
            {
                ids.push(id.clone());
            }
            if let Some(Value::String(node_id)) = obj.get("node_id")
                && !node_id.is_empty()
                && !ids.contains(node_id)
            {
                ids.push(node_id.clone());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if let Value::Object(obj) = item
                    && let Some(Value::String(id)) = obj.get("id")
                    && !id.is_empty()
                {
                    ids.push(id.clone());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_object() {
        let body = br#"{"id": 123, "node_id": "MDExOlB1bGxSZXF1ZXN0MQ==", "title": "Test"}"#;
        let ids = extract_entity_ids(body);
        assert_eq!(ids, vec!["MDExOlB1bGxSZXF1ZXN0MQ=="]);
    }

    #[test]
    fn test_extract_array() {
        let body = br#"[
            {"id": 1, "node_id": "AAA", "title": "Issue 1"},
            {"id": 2, "node_id": "BBB", "title": "Issue 2"}
        ]"#;
        let ids = extract_entity_ids(body);
        assert_eq!(ids, vec!["AAA", "BBB"]);
    }

    #[test]
    fn test_extract_no_node_id() {
        let body = br#"{"id": 123, "message": "Not Found"}"#;
        let ids = extract_entity_ids(body);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_malformed_json() {
        let body = b"not json at all";
        let ids = extract_entity_ids(body);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_empty_body() {
        let ids = extract_entity_ids(b"");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_graphql_single_entity() {
        let body = br#"{
            "data": {
                "repository": {
                    "id": "MDEwOlJlcG9zaXRvcnkx",
                    "name": "test-repo"
                }
            }
        }"#;
        let ids = extract_graphql_entity_ids(body);
        assert_eq!(ids, vec!["MDEwOlJlcG9zaXRvcnkx"]);
    }

    #[test]
    fn test_graphql_nested_entity() {
        let body = br#"{
            "data": {
                "repository": {
                    "id": "REPO_ID",
                    "issue": {
                        "id": "ISSUE_ID",
                        "title": "Bug"
                    }
                }
            }
        }"#;
        // Top-level traversal: only "repository" is checked at data level
        let ids = extract_graphql_entity_ids(body);
        assert_eq!(ids, vec!["REPO_ID"]);
    }

    #[test]
    fn test_graphql_array_in_data() {
        let body = br#"{
            "data": {
                "search": {
                    "nodes": [
                        {"id": "NODE_A"},
                        {"id": "NODE_B"}
                    ]
                }
            }
        }"#;
        // search object doesn't have id, nodes is an array at second level
        let ids = extract_graphql_entity_ids(body);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_graphql_no_data() {
        let body = br#"{"errors": [{"message": "Bad query"}]}"#;
        let ids = extract_graphql_entity_ids(body);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_graphql_malformed() {
        let ids = extract_graphql_entity_ids(b"not json");
        assert!(ids.is_empty());
    }
}
