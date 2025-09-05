use chemengine::ChemEngine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::DomainError;
use std::fmt;

static ENGINE: Lazy<ChemEngine> = Lazy::new(|| {
    ChemEngine::init().expect("Failed to initialize ChemEngine")
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Molecule {
    inchikey: String,
    smiles: String,
    inchi: String,
    metadata: serde_json::Value,
}

impl Molecule {
     fn new(inchikey: &str, smiles: &str, inchi: &str, metadata: serde_json::Value) -> Result<Self, DomainError> {
        let normalized_inchikey = inchikey.to_uppercase();
        if normalized_inchikey.len() != 27 || normalized_inchikey.matches('-').count() < 2 {
            return Err(DomainError::ValidationError("Invalid InChIKey format".to_string()));
        }
        Ok(Molecule {
            inchikey: normalized_inchikey,
            smiles: smiles.to_string(),
            inchi: inchi.to_string(),
            metadata,
        })
    }
    pub fn new_molecule_with_smiles(smiles: &str) -> Result<Self, DomainError> {
        let chem_molecule = ENGINE.get_molecule(smiles)?;
        Molecule::new(
            &chem_molecule.inchikey,
            &chem_molecule.smiles,
            &chem_molecule.inchi,
            serde_json::json!("create rdkit molecule from smiles"),
        )
    }
    pub fn smiles(&self) -> &str { &self.smiles }
    pub fn inchikey(&self) -> &str { &self.inchikey }
    pub fn inchi(&self) -> &str { &self.inchi }
    pub fn compare(&self, other: &Molecule) -> bool { self.inchikey == other.inchikey }
}
 
impl fmt::Display for Molecule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<smile: {}, {}>", self.smiles, self.inchi)
    }
}
