//! Utilidades para fusionar parámetros JSON de forma determinista.
//!
//! Aquí implementamos un merge "shallow" donde las claves de `b` reemplazan
//! a las de `a`. Para objetos anidados se puede extender a `deep-merge` si
//! es necesario; por ahora mantenemos la semántica simple y predecible.

use serde_json::Value;

/// Merge shallow: keys from `b` override keys from `a` when both are objects.
/// Cuando alguno de los dos valores no es objeto, `b` tiene precedencia.
pub fn merge_json(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut out = ma.clone();
            for (k, v) in mb.iter() {
                out.insert(k.clone(), v.clone());
            }
            Value::Object(out)
        }
        // Non-objects: override
        (_, other) => other.clone(),
    }
}
