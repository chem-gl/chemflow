use crate::error::DomainError;
use serde::{Deserialize, Serialize};

/// INV1: inchikey único.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Molecule {
    inchikey: String,
    smiles: String,
    inchi: String,
    metadata: serde_json::Value,
}

impl Molecule {
    pub fn new(inchikey: &str, smiles: &str, inchi: &str, metadata: serde_json::Value) -> Result<Self, DomainError> {
        let normalized_inchikey = inchikey.to_uppercase();
        if normalized_inchikey.len() != 27 || normalized_inchikey.matches('-').count() < 2 {
            return Err(DomainError::ValidationError("Invalid InChIKey format".to_string()));
        }

        Ok(Molecule { inchikey: normalized_inchikey,
                      smiles: smiles.to_string(),
                      inchi: inchi.to_string(),
                      metadata })
    }
    pub fn to_string(&self) -> String {
        self.inchikey.clone()
    }
    pub fn inchikey(&self) -> &str {
        return &self.inchikey;
    }
}
