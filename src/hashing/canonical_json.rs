use serde_json::{Value};
use std::collections::BTreeMap;

/// Serializa un `Value` de JSON a una representación canónica:
/// - Objetos con claves ordenadas
/// - Sin espacios redundantes
pub fn to_canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => serde_json::to_string(s).unwrap(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(to_canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(map) => {
            let mut tree = BTreeMap::new();
            for (k, v) in map {
                tree.insert(k, to_canonical_json(v));
            }
            let items: Vec<String> = tree.into_iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(&k).unwrap(), v))
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::to_canonical_json;
    use serde_json::json;

    #[test]
    fn test_primitives() {
        assert_eq!(to_canonical_json(&json!(null)), "null");
        assert_eq!(to_canonical_json(&json!(true)), "true");
        assert_eq!(to_canonical_json(&json!(123)), "123");
        assert_eq!(to_canonical_json(&json!("hola")), "\"hola\"");
    }

    #[test]
    fn test_array() {
        let val = json!([3, "a", false]);
        assert_eq!(to_canonical_json(&val), "[3,\"a\",false]");
    }

    #[test]
    fn test_object_sorted_keys() {
        let val = json!({ "b": 2, "a": 1 });
        assert_eq!(to_canonical_json(&val), "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn test_nested() {
        let val = json!({ "z": [ { "y": "yes" }, null ], "a": { "x": 10 } });
        let canonical = to_canonical_json(&val);
        // keys sorted: a then z; nested object x
        assert_eq!(canonical, "{\"a\":{\"x\":10},\"z\":[{\"y\":\"yes\"},null]}");
    }
}
