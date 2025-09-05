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
    pub hash: String,               // hash canonical del payload
    pub payload: Value,             // contenido neutro
    pub metadata: Option<Value>,    // opcional; no entra al hash del payload (solo payload)
}

impl Artifact {
    /// Constructor helper – no calcula hash (lo hará FlowEngine en el futuro).
    pub fn new_unhashed(kind: ArtifactKind, payload: Value, metadata: Option<Value>) -> Self {
        Self { kind, hash: String::new(), payload, metadata }
    }
}
