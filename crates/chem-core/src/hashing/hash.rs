//! Hash helpers – abstracción para permitir cambiar de algoritmo sin tocar
//! resto del core.
//!
//! Actualmente se usa BLAKE3 por su rendimiento y propiedades prácticas. El
//! tipo de hash se encapsula para poder migrar a otro algoritmo en el futuro
//! sin romper APIs del engine.

use crate::hashing::to_canonical_json;
use blake3::Hasher;
use serde_json::Value;

/// Hashea un string y devuelve hex.
pub fn hash_str(input: &str) -> String {
    let mut h = Hasher::new();
    h.update(input.as_bytes());
    h.finalize().to_hex().to_string()
}

/// Hashea un JSON Value aplicando primero canonicalización.
pub fn hash_value(v: &Value) -> String {
    let canonical = to_canonical_json(v);
    hash_str(&canonical)
}
