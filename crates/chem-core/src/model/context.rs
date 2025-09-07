//! Contexto de ejecución entregado a cada Step.
//!
//! El `ExecutionContext` encapsula el único artifact de entrada (si existe)
//! y los parámetros canonicalizados. Los helpers permiten decodificar ambos a
//! tipos fuertes cuando se usa la infraestructura tipada.
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::Artifact;
use crate::model::{ArtifactSpec, TypedArtifact};

/// Contexto de ejecución entregado al StepDefinition::run
pub struct ExecutionContext {
    pub input: Option<Artifact>, // Artifact único encadenado (None primer step)
    pub params: Value,           // parámetros canónicos
}

impl ExecutionContext {
    /// Decodifica los parámetros del step a un tipo fuerte usando serde.
    /// Útil para evitar acceder por strings en JSON.
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.params.clone())
    }

    /// Decodifica el artifact de entrada como un tipo fuerte `T`.
    /// Devuelve error si no hay input o si el artifact no coincide con el spec
    /// (kind/versión/validación).
    pub fn input_typed<T: ArtifactSpec + Clone>(&self) -> Result<TypedArtifact<T>, String> {
        let a = self.input
                    .as_ref()
                    .ok_or_else(|| "ExecutionContext.input is None (primer step o falta de salida previa)".to_string())?;
        TypedArtifact::<T>::decode(a).map_err(|e| format!("TypedArtifact decode error: {:?}", e))
    }
}
