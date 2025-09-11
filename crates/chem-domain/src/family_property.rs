// family_property.rs
use crate::{DomainError, MoleculeFamily};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Representa una propiedad calculada para una familia molecular completa
/// con verificación de integridad mediante hash. Es genérica en el tipo de
/// valor y metadatos, permitiendo flexibilidad en los tipos de propiedades
/// almacenadas.
#[derive(Debug, Clone)]
pub struct FamilyProperty<'a, ValueType, TypeMeta> {
    id: Uuid,
    family: &'a MoleculeFamily,
    property_type: String,
    value: ValueType,
    quality: Option<String>,
    preferred: bool,
    value_hash: String,
    metadata: TypeMeta,
}

impl<'a, ValueType, TypeMeta> FamilyProperty<'a, ValueType, TypeMeta>
    where ValueType: Serialize + Clone,
          TypeMeta: Serialize + Clone
{
    /// Crea una nueva propiedad de familia con validaciones exhaustivas
    ///
    /// # Argumentos
    /// * `family` - Referencia a la familia molecular
    /// * `property_type` - Tipo de propiedad (ej: "average_logP",
    ///   "diversity_index")
    /// * `value` - Valor de la propiedad
    /// * `quality` - Calidad o confianza de la propiedad (opcional)
    /// * `preferred` - Indica si esta es la propiedad preferida entre varias
    ///   del mismo tipo
    /// * `metadata` - Metadatos específicos del tipo de propiedad
    ///
    /// # Errores
    /// Retorna `DomainError::ValidationError` si el tipo de propiedad está
    /// vacío Retorna `DomainError::SerializationError` si hay errores al
    /// serializar
    pub fn new(family: &'a MoleculeFamily,
               property_type: &str,
               value: ValueType,
               quality: Option<String>,
               preferred: bool,
               metadata: TypeMeta)
               -> Result<Self, DomainError> {
        // Validar que el tipo de propiedad no esté vacío
        if property_type.trim().is_empty() {
            return Err(DomainError::ValidationError("El tipo de propiedad no puede estar vacío".to_string()));
        }

        // Crear hasher para verificación de integridad
        let mut hasher = Sha256::new();

        // Incluir identificador único de la familia en el hash
        hasher.update(family.family_hash().as_bytes());
        hasher.update(property_type.as_bytes());

        // Serializar valor para incluirlo en el hash
        let value_json = serde_json::to_string(&value).map_err(|e| DomainError::SerializationError(e.to_string()))?;
        hasher.update(value_json.as_bytes());

        // Serializar metadatos para incluirlos en el hash
        let metadata_json = serde_json::to_string(&metadata).map_err(|e| DomainError::SerializationError(e.to_string()))?;
        hasher.update(metadata_json.as_bytes());

        // Generar hash final
        let value_hash = format!("{:x}", hasher.finalize());

        Ok(FamilyProperty { id: Uuid::new_v4(),
                            family,
                            property_type: property_type.to_string(),
                            value,
                            quality,
                            preferred,
                            value_hash,
                            metadata })
    }

    /// Obtiene el ID único de la propiedad
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Obtiene el ID de la familia asociada a esta propiedad
    pub fn family_id(&self) -> Uuid {
        self.family.id()
    }

    /// Constructor rápido que usa metadata vacío y valores por defecto para
    /// quality/preferred
    pub fn quick_new(family: &'a MoleculeFamily, property_type: &str, value: ValueType) -> Result<Self, DomainError>
        where TypeMeta: Default
    {
        Self::new(family, property_type, value, None, false, TypeMeta::default())
    }

    /// Obtiene la familia molecular asociada a esta propiedad
    pub fn family(&self) -> &MoleculeFamily {
        self.family
    }

    /// Obtiene el tipo de propiedad
    pub fn property_type(&self) -> &str {
        &self.property_type
    }

    /// Obtiene el valor de la propiedad
    pub fn value(&self) -> &ValueType {
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
    pub fn metadata(&self) -> &TypeMeta {
        &self.metadata
    }

    /// Obtiene el hash único que identifica esta propiedad específica
    pub fn value_hash(&self) -> &str {
        &self.value_hash
    }

    /// Crea una nueva instancia con calidad modificada
    pub fn with_quality(&self, quality: Option<String>) -> Result<Self, DomainError> {
        Self::new(self.family,
                  &self.property_type,
                  self.value.clone(),
                  quality,
                  self.preferred,
                  self.metadata.clone())
    }

    /// Crea una nueva instancia con metadatos modificados
    pub fn with_metadata(&self, metadata: TypeMeta) -> Result<Self, DomainError> {
        Self::new(self.family,
                  &self.property_type,
                  self.value.clone(),
                  self.quality.clone(),
                  self.preferred,
                  metadata)
    }

    /// Crea una nueva instancia con el flag 'preferred' modificado
    pub fn with_preferred(&self, preferred: bool) -> Result<Self, DomainError> {
        Self::new(self.family,
                  &self.property_type,
                  self.value.clone(),
                  self.quality.clone(),
                  preferred,
                  self.metadata.clone())
    }

    /// Verifica si dos propiedades son equivalentes comparando sus hashes
    pub fn is_equivalent(&self, other: &FamilyProperty<'a, ValueType, TypeMeta>) -> bool {
        self.value_hash == other.value_hash
    }

    /// Verifica la integridad de la propiedad recalculando y comparando el hash
    pub fn verify_integrity(&self) -> Result<bool, DomainError> {
        let mut hasher = Sha256::new();
        hasher.update(self.family.family_hash().as_bytes());
        hasher.update(self.property_type.as_bytes());

        let value_json = serde_json::to_string(&self.value).map_err(|e| DomainError::SerializationError(e.to_string()))?;
        hasher.update(value_json.as_bytes());

        let metadata_json =
            serde_json::to_string(&self.metadata).map_err(|e| DomainError::SerializationError(e.to_string()))?;
        hasher.update(metadata_json.as_bytes());

        let calculated_hash = format!("{:x}", hasher.finalize());

        Ok(calculated_hash == self.value_hash)
    }
}

// Implementación de Display para formato legible
impl<'a, ValueType, TypeMeta> fmt::Display for FamilyProperty<'a, ValueType, TypeMeta>
    where ValueType: Serialize + Clone + fmt::Debug,
          TypeMeta: Serialize + Clone + fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
               "FamilyProperty(id: {}, type: {}, preferred: {})",
               self.id, self.property_type, self.preferred)
    }
}

// Implementación de PartialEq basada en el hash de valor
impl<'a, ValueType, TypeMeta> PartialEq for FamilyProperty<'a, ValueType, TypeMeta>
    where ValueType: Serialize + Clone,
          TypeMeta: Serialize + Clone
{
    fn eq(&self, other: &Self) -> bool {
        self.is_equivalent(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_family_property_creation() -> Result<(), DomainError> {
        // Crear una familia de prueba
        let mol1 = crate::Molecule::from_smiles("CCO")?;
        let mol2 = crate::Molecule::from_smiles("CCN")?;
        let provenance = json!({"source": "test"});
        let family = crate::MoleculeFamily::new(vec![mol1, mol2], provenance)?;

        // Crear una propiedad para la familia
        let metadata = json!({"calculation_method": "test"});
        let property = FamilyProperty::new(&family, "average_logP", 2.5, Some("high".to_string()), true, metadata)?;

        assert_eq!(property.property_type(), "average_logP");
        assert_eq!(property.value(), &2.5);
        assert!(property.verify_integrity()?);
        Ok(())
    }

    #[test]
    fn test_family_property_equivalence() -> Result<(), DomainError> {
        // Crear una familia de prueba
        let mol1 = crate::Molecule::from_smiles("CCO")?;
        let mol2 = crate::Molecule::from_smiles("CCN")?;
        let provenance = json!({"source": "test"});
        let family = crate::MoleculeFamily::new(vec![mol1, mol2], provenance)?;

        // Crear dos propiedades idénticas
        let metadata = json!({"calculation_method": "test"});
        let prop1 = FamilyProperty::new(&family, "average_logP", 2.5, Some("high".to_string()), true, metadata.clone())?;

        let prop2 = FamilyProperty::new(&family, "average_logP", 2.5, Some("high".to_string()), true, metadata)?;

        // Deben ser equivalentes
        assert_eq!(prop1, prop2);
        Ok(())
    }

    #[test]
    fn test_family_property_empty_type() {
        // Crear una familia de prueba
        let mol1 = crate::Molecule::from_smiles("CCO").unwrap();
        let mol2 = crate::Molecule::from_smiles("CCN").unwrap();
        let provenance = json!({"source": "test"});
        let family = crate::MoleculeFamily::new(vec![mol1, mol2], provenance).unwrap();

        // Intentar crear propiedad con tipo vacío debe fallar
        let metadata = json!({"calculation_method": "test"});
        let result = FamilyProperty::new(&family, "", 2.5, Some("high".to_string()), true, metadata);

        assert!(result.is_err());
    }
}
