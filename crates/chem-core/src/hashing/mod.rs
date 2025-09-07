//! Hashing y canonicalización JSON.
//!
//! Papel en el flujo:
//! - La reproducción determinista depende de serializaciones canónicas.
//! - `to_canonical_json` garantiza orden estable para objetos JSON.
//! - `hash_str` y `hash_value` producen identificadores estables para artifacts
//!   y fingerprints.

pub mod canonical_json;
pub mod hash;

pub use canonical_json::to_canonical_json;
pub use hash::{hash_str, hash_value};
