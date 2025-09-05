//! Canonical JSON minimal – mueve lógica desde crate raíz (sin dependencias de dominio).
//! TODO: Optimizar para rendimiento y soportar números edge.

use serde_json::Value;
use std::collections::BTreeMap;

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
            let items: Vec<String> = tree
                .into_iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(&k).unwrap(), v))
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}
