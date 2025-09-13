// Reexport de la implementación única ubicada en `chem-core` para evitar
// duplicación de lógica de canonicalización/hashing a nivel de workspace.
pub use chem_core::hashing::canonical_json;
pub use chem_core::hashing::canonical_json::to_canonical_json;
