//! Artifact neutral del flujo.
//!
//! Un `Artifact` es la unidad de datos intercambiada entre steps. Es neutral:
//! - `payload` es JSON genérico; el motor no interpreta su semántica.
//! - `hash` es calculado por el engine sobre el JSON canonicalizado (ver
//!   `hashing::to_canonical_json`). Este hash sirve como identidad para
//!   deduplicación y trazabilidad de outputs.
//! - `metadata` permite anotar información auxiliar que no entra al hash.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tipos neutrales de artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// JSON genérico sin semántica (válido para F2)
    GenericJson,
}

/// Artifact neutral producido/consumido por Steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub hash: String,            // hash canonical del payload (asignado por engine)
    pub payload: Value,          // contenido neutro JSON
    pub metadata: Option<Value>, // información auxiliar (no entra al hash)
}

impl Artifact {
    /// Constructor interno; preferir crear artifacts a través de
    /// `ArtifactSpec::into_artifact`.
    pub(crate) fn new_unhashed(kind: ArtifactKind, payload: Value, metadata: Option<Value>) -> Self {
        Self { kind,
               hash: String::new(),
               payload,
               metadata }
    }
}
