use crate::error::DomainError;
use chemengine::ChemEngine;
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
    fn new(inchikey: &str, smiles: &str, inchi: &str, metadata: serde_json::Value) -> Result<Self, DomainError> {
        let normalized_inchikey = inchikey.to_uppercase();
        if normalized_inchikey.len() != 27 || normalized_inchikey.matches('-').count() < 2 {
            return Err(DomainError::ValidationError("Invalid InChIKey format".to_string()));
        }
        Ok(Molecule { 
                    inchikey: normalized_inchikey,
                    smiles: smiles.to_string(),
                    inchi: inchi.to_string(),
                    metadata: serde_json::Value::Object(serde_json::Map::new(
                    ))
                })
    }
    pub fn new_molecule_with_smiles(smiles: &str) -> Result<Self, DomainError> {
        let engine = ChemEngine::init();
        match engine {
            Ok(engine) => {
                let mol = engine.get_molecule(smiles);
                match mol {
                    Ok(chem_molecule) => {
                        Molecule::new(
                            &chem_molecule.inchikey,
                            &chem_molecule.smiles,
                            &chem_molecule.inchi,
                    serde_json::json!("create rdkit molecule from smiles")
                        )
                    },
                    Err(e) => Err(DomainError::ExternalError(format!("Error obteniendo molécula: {:?}", e))),
                }
            }
            Err(e) => Err(DomainError::ExternalError(format!("Error inicializando motor químico: {:?}", e))),
        }
    }



    pub fn smiles(&self) -> &str {
        &self.smiles
    }
    pub fn to_string(&self) -> String {
        self.inchikey.clone()
    }
    pub fn inchikey(&self) -> &str {
        return &self.inchikey;
    }
}
