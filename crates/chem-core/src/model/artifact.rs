use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tipos neutrales de artifact. Extensible; no incluir semántica química.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// JSON genérico sin semántica (válido para F2)
    GenericJson,
    // TODO: Añadir más tipos neutrales (BinaryBlob, Metrics, etc.) en futuras fases.
}

/// Artifact neutral producido/consumido por Steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub hash: String,               // hash canonical del payload (asignado por engine)
    pub payload: Value,             // contenido neutro JSON
    pub metadata: Option<Value>,    // información auxiliar (no entra al hash)
}

impl Artifact {
    /// Constructor interno; preferir crear artifacts a través de `ArtifactSpec::into_artifact`.
    pub(crate) fn new_unhashed(kind: ArtifactKind, payload: Value, metadata: Option<Value>) -> Self {
        Self { kind, hash: String::new(), payload, metadata }
    }
}
