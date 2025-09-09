use chem_core::hashing::hash_value;
use serde_json::json;

#[test]
fn hash_value_produces_hex_64() {
    let v = json!({"b":2, "a":1});
    let h = hash_value(&v);
    // blake3 hex length is 64
    assert_eq!(h.len(), 64);
    // deterministic: same value with different key order yields same hash
    let v2 = json!({"a":1, "b":2});
    let h2 = hash_value(&v2);
    assert_eq!(h, h2);
}
