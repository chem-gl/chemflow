//! Pruebas para utilitarios de merge JSON (inyección de params)
//!
//! Verificamos la semántica shallow: claves de `b` sobreescriben claves de `a`.

use chem_core::injection::merge_json;
use serde_json::json;

#[test]
fn merge_shallow_overrides_keys() {
    let a = json!({"x": 1, "y": {"z": 3}, "keep": "a"});
    let b = json!({"x": 2, "y": "replaced", "new": true});

    let out = merge_json(&a, &b);

    // claves simples son sobreescritas
    assert_eq!(out["x"], json!(2));
    // cuando b tiene un tipo no-objeto, reemplaza completamente
    assert_eq!(out["y"], json!("replaced"));
    // claves que sólo existen en a se mantienen
    assert_eq!(out["keep"], json!("a"));
    // claves nuevas en b aparecen
    assert_eq!(out["new"], json!(true));
}
