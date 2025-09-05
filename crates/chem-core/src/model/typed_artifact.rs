//! Infraestructura opcional de tipado fuerte para `Artifact` manteniendo el núcleo agnóstico.
//! Permite describir artefactos con un tipo de datos concreto (T) y validaciones.
//! No introduce semántica de dominio; se basa en generics y serde.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use super::{Artifact, ArtifactKind};

/// Errores posibles al decodificar un artifact tipado.
#[derive(Debug)]
pub enum ArtifactDecodeError {
    KindMismatch { expected: ArtifactKind, found: ArtifactKind },
    VersionMismatch { expected: u32, found: Option<u32> },
    Deserialize(String),
    Validation(String),
}

/// Especificación abstracta de un artifact tipado.
/// Implementado por tipos de datos que quieren exponerse como artifacts seguros.
pub trait ArtifactSpec: Sized + Serialize + DeserializeOwned + Clone {
    /// Kind asociado (permite distinguir en runtime).
    const KIND: ArtifactKind;
    /// Versión de esquema (incrementar en cambios incompatibles).
    const SCHEMA_VERSION: u32 = 1;

    /// Validación semántica ligera (sin efectos secundarios). Opcional.
    fn validate(&self) -> Result<(), String> { Ok(()) }

    /// Nombre de campo que llevará la versión dentro del payload. Por defecto `schema_version`.
    /// Puede modificarse si el tipo ya usa ese nombre.
    fn version_field_name() -> &'static str { "schema_version" }

    /// Serializa a `Artifact` sin hash (lo añadirá el engine).
    fn into_artifact(self) -> Artifact {
        let mut value = serde_json::to_value(&self).expect("serialize artifact spec");
        // Insertar versión si no existe.
        if let Value::Object(map) = &mut value {
            map.entry(Self::version_field_name().to_string()).or_insert(Value::from(Self::SCHEMA_VERSION));
        }
        Artifact::new_unhashed(Self::KIND, value, None)
    }

    /// Decodifica desde artifact neutro verificando kind, versión y validación.
    fn from_artifact(a: &Artifact) -> Result<Self, ArtifactDecodeError> {
        if a.kind != Self::KIND {
            return Err(ArtifactDecodeError::KindMismatch { expected: Self::KIND, found: a.kind.clone() });
        }
        // Extraer versión
        let found_version = a.payload.get(Self::version_field_name()).and_then(|v| v.as_u64()).map(|v| v as u32);
        if let Some(v) = found_version {
            if v != Self::SCHEMA_VERSION {
                return Err(ArtifactDecodeError::VersionMismatch { expected: Self::SCHEMA_VERSION, found: Some(v) });
            }
        } else {
            return Err(ArtifactDecodeError::VersionMismatch { expected: Self::SCHEMA_VERSION, found: None });
        }
        let mut cloned = a.payload.clone();
        // Eliminar campo de versión antes de deserializar si no está en struct.
        if let Value::Object(map) = &mut cloned {
            // Si el struct define el campo lo ignorará; si no, preferimos dejarlo presente (serde ignora extra) – conservamos.
            let _ = map; // placeholder
        }
        let decoded: Self = serde_json::from_value(cloned)
            .map_err(|e| ArtifactDecodeError::Deserialize(e.to_string()))?;
        decoded.validate().map_err(ArtifactDecodeError::Validation)?;
        Ok(decoded)
    }
}

/// Adaptador genérico para un artifact tipado ya decodificado (útil en runtimes polimórficos).
pub struct TypedArtifact<T: ArtifactSpec> {
    pub inner: T,
    pub raw: Artifact, // mantiene representación original (hash incluido cuando se calcule)
}

impl<T: ArtifactSpec + Clone> TypedArtifact<T> {
    pub fn new(inner: T) -> Self { Self { raw: inner.clone().into_artifact(), inner } }

    pub fn decode(raw: &Artifact) -> Result<Self, ArtifactDecodeError> {
        let inner = T::from_artifact(raw)?;
        Ok(Self { inner, raw: raw.clone() })
    }
}
