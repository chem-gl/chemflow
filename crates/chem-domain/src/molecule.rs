// molecule.rs
use crate::DomainError;
use chemengine::ChemEngine;
use chrono;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Inicialización segura del motor químico con manejo de errores
static ENGINE: Lazy<Result<ChemEngine, DomainError>> = Lazy::new(|| {
    ChemEngine::init().map_err(|e| DomainError::ExternalError(format!("Error al inicializar el motor químico: {}", e)))
});

/// Representa una molécula química con sus identificadores únicos y metadatos
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Molecule {
    inchikey: String,
    smiles: String,
    inchi: String,
    metadata: serde_json::Value,
}

impl Molecule {
    /// Crea una nueva molécula con validaciones exhaustivas
    fn new(inchikey: &str, smiles: &str, inchi: &str, metadata: serde_json::Value) -> Result<Self, DomainError> {
        // Normalizar y validar el formato InChIKey
        let normalized_inchikey = inchikey.to_uppercase();

        // Verificar longitud exacta
        if normalized_inchikey.len() != 27 {
            return Err(DomainError::ValidationError("InChIKey debe tener exactamente 27 caracteres".to_string()));
        }

        // Verificar número de separadores
        if normalized_inchikey.matches('-').count() != 2 {
            return Err(DomainError::ValidationError("InChIKey debe contener exactamente dos guiones".to_string()));
        }

        // Validar formato de las secciones del InChIKey
        let parts: Vec<&str> = normalized_inchikey.split('-').collect();
        if parts.len() != 3 {
            return Err(DomainError::ValidationError("Formato InChIKey inválido".to_string()));
        }

        // Validar caracteres de cada sección
        if !parts[0].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
           || !parts[1].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
           || !parts[2].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(DomainError::ValidationError("InChIKey contiene caracteres inválidos".to_string()));
        }

        // Validar que SMILES no esté vacío
        if smiles.trim().is_empty() {
            return Err(DomainError::ValidationError("SMILES no puede estar vacío".to_string()));
        }

        // Validar que InChI no esté vacío
        if inchi.trim().is_empty() {
            return Err(DomainError::ValidationError("InChI no puede estar vacío".to_string()));
        }

        Ok(Molecule { inchikey: normalized_inchikey,
                      smiles: smiles.to_string(),
                      inchi: inchi.to_string(),
                      metadata })
    }

    /// Crea una molécula a partir de las partes ya conocidas (InChIKey, SMILES,
    /// InChI). Útil para pruebas donde el motor químico (RDKit) no está
    /// disponible.
    pub fn from_parts(inchikey: &str, smiles: &str, inchi: &str, metadata: serde_json::Value) -> Result<Self, DomainError> {
        Self::new(inchikey, smiles, inchi, metadata)
    }

    pub fn from_smiles(smiles: &str) -> Result<Self, DomainError> {
        // Verificar entrada vacía
        if smiles.trim().is_empty() {
            return Err(DomainError::ValidationError("SMILES de entrada no puede estar vacío".to_string()));
        }
        // Obtener instancia del motor con manejo de errores
        let engine = ENGINE.as_ref()
                           .map_err(|e| DomainError::ExternalError(format!("Motor químico no disponible: {}", e)))?;

        // Obtener molécula del motor
        let chem_molecule = engine.get_molecule(smiles)
                                  .map_err(|e| DomainError::ExternalError(format!("Error al procesar SMILES: {}", e)))?;

        // Crear instancia con metadatos relevantes
        Self::new(&chem_molecule.inchikey,
                  &chem_molecule.smiles,
                  &chem_molecule.inchi,
                  serde_json::json!({
                      "source": "created_from_smiles",
                      "original_smiles": smiles,
                      "generation_timestamp": Utc::now().to_rfc3339(),
                  }))
    }

    /// Obtiene el SMILES de la molécula
    pub fn smiles(&self) -> &str {
        &self.smiles
    }

    /// Obtiene el InChIKey de la molécula
    pub fn inchikey(&self) -> &str {
        &self.inchikey
    }

    /// Obtiene el InChI de la molécula
    pub fn inchi(&self) -> &str {
        &self.inchi
    }

    /// Obtiene los metadatos de la molécula
    pub fn metadata(&self) -> &serde_json::Value {
        &self.metadata
    }

    /// Compara si dos moléculas son la misma basándose en el InChIKey
    pub fn is_same(&self, other: &Molecule) -> bool {
        self.inchikey == other.inchikey
    }
}

// Implementación de Display para formato legible
impl fmt::Display for Molecule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
               "Molecule(SMILES: {}, InChI: {}, InChIKey: {})",
               self.smiles, self.inchi, self.inchikey)
    }
}
