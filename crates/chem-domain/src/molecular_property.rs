// molecular_property.rs
use crate::{DomainError, Molecule};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Representa una propiedad molecular con metadatos y capacidad de verificación
/// de integridad mediante hash. Es genérica en el tipo de valor y metadatos.
#[derive(Debug, Clone)]
pub struct MolecularProperty<'a, TypeValue, TypeMetaData> {
    id: Uuid,
    molecule: &'a Molecule,
    property_type: String,
    value: TypeValue,
    quality: Option<String>,
    preferred: bool,
    value_hash: String,
    metadata: TypeMetaData,
}

impl<'a, TypeValue, TypeMetaData> MolecularProperty<'a, TypeValue, TypeMetaData>
    where TypeValue: Serialize + Clone,
          TypeMetaData: Serialize + Clone
{
    /// Crea una nueva propiedad molecular con validaciones exhaustivas
    pub fn new(molecule: &'a Molecule,
               property_type: &str,
               value: TypeValue,
               quality: Option<String>,
               preferred: bool,
               metadata: TypeMetaData)
               -> Result<Self, DomainError> {
        // Validar que el tipo de propiedad no esté vacío
        if property_type.trim().is_empty() {
            return Err(DomainError::ValidationError("El tipo de propiedad no puede estar vacío".to_string()));
        }

        // Crear hasher para verificación de integridad
        let mut hasher = Sha256::new();

        // Incluir identificadores únicos de la molécula en el hash
        hasher.update(molecule.inchikey().as_bytes());
        hasher.update(property_type.as_bytes());

        // Serializar valor para incluirlo en el hash
        let value_json =
            serde_json::to_string(&value).map_err(|e| {
                                             DomainError::ExternalError(format!("Error al serializar valor: {}", e))
                                         })?;
        hasher.update(value_json.as_bytes());

        // Serializar metadatos para incluirlos en el hash
        let metadata_json =
            serde_json::to_string(&metadata).map_err(|e| {
                                                DomainError::ExternalError(format!("Error al serializar metadatos: {}", e))
                                            })?;
        hasher.update(metadata_json.as_bytes());
        // Generar hash final
        let value_hash = format!("{:x}", hasher.finalize());
        Ok(MolecularProperty { id: Uuid::new_v4(),
                               molecule,
                               property_type: property_type.to_string(),
                               value,
                               quality,
                               preferred,
                               value_hash,
                               metadata })
    }

    /// Obtiene el hash único que identifica esta propiedad específica
    pub fn value_hash(&self) -> &str {
        &self.value_hash
    }

    /// Obtiene el ID único de la propiedad
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Obtiene la molécula asociada a esta propiedad
    pub fn molecule(&self) -> &Molecule {
        self.molecule
    }

    /// Obtiene el tipo de propiedad (ej: "logP", "polar_surface_area")
    pub fn property_type(&self) -> &str {
        &self.property_type
    }

    /// Obtiene el valor de la propiedad
    pub fn value(&self) -> &TypeValue {
        &self.value
    }

    /// Obtiene la calidad de la propiedad si está disponible
    pub fn quality(&self) -> Option<&String> {
        self.quality.as_ref()
    }

    /// Indica si esta es la propiedad preferida entre varias del mismo tipo
    pub fn preferred(&self) -> bool {
        self.preferred
    }

    /// Obtiene los metadatos específicos del tipo de propiedad
    pub fn metadata(&self) -> &TypeMetaData {
        &self.metadata
    }

    /// Crea una nueva instancia con calidad modificada
    pub fn with_quality(&self, quality: Option<String>) -> Result<Self, DomainError> {
        Self::new(self.molecule,
                  &self.property_type,
                  self.value.clone(),
                  quality,
                  self.preferred,
                  self.metadata.clone())
    }

    /// Crea una nueva instancia con metadatos modificados
    pub fn with_metadata(&self, metadata: TypeMetaData) -> Result<Self, DomainError> {
        Self::new(self.molecule,
                  &self.property_type,
                  self.value.clone(),
                  self.quality.clone(),
                  self.preferred,
                  metadata)
    }

    /// Crea una nueva instancia con el flag 'preferred' modificado
    pub fn with_preferred(&self, preferred: bool) -> Result<Self, DomainError> {
        Self::new(self.molecule,
                  &self.property_type,
                  self.value.clone(),
                  self.quality.clone(),
                  preferred,
                  self.metadata.clone())
    }

    /// Verifica si dos propiedades son equivalentes comparando sus hashes
    pub fn is_equivalent(&self, other: &MolecularProperty<'a, TypeValue, TypeMetaData>) -> bool {
        self.value_hash == other.value_hash
    }
    /// Verifica la integridad de la propiedad recalculando y comparando el hash
    pub fn verify_integrity(&self) -> Result<bool, DomainError> {
        let mut hasher = Sha256::new();
        hasher.update(self.molecule.inchikey().as_bytes());
        hasher.update(self.property_type.as_bytes());
        let value_json =
            serde_json::to_string(&self.value).map_err(|e| {
                                                  DomainError::ExternalError(format!("Error al serializar valor: {}", e))
                                              })?;
        hasher.update(value_json.as_bytes());

        let metadata_json =
            serde_json::to_string(&self.metadata).map_err(|e| {
                                                     DomainError::ExternalError(format!("Error al serializar metadatos: {}",
                                                                                        e))
                                                 })?;
        hasher.update(metadata_json.as_bytes());

        let calculated_hash = format!("{:x}", hasher.finalize());

        Ok(calculated_hash == self.value_hash)
    }
}
// Implementación de Display para formato legible
impl<'a, TypeValue, TypeMetaData> fmt::Display for MolecularProperty<'a, TypeValue, TypeMetaData>
    where TypeValue: Serialize + Clone + fmt::Debug,
          TypeMetaData: Serialize + Clone + fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
               "MolecularProperty(id: {}, type: {}, preferred: {})",
               self.id, self.property_type, self.preferred)
    }
}

// Implementación de PartialEq basada en el hash de valor
impl<'a, TypeValue, TypeMetaData> PartialEq for MolecularProperty<'a, TypeValue, TypeMetaData>
    where TypeValue: Serialize + Clone,
          TypeMetaData: Serialize + Clone
{
    fn eq(&self, other: &Self) -> bool {
        self.is_equivalent(other)
    }
}
