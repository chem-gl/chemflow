use serde::{Serialize, Deserialize};
use uuid::Uuid;
use sha2::{Sha256, Digest};

use crate::Molecule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MolecularProperty {
    id: Uuid,
    molecule: String,
    name: String,
    value: serde_json::Value,
    units: Option<String>,
    quality: Option<String>,
    preferred: bool,
    value_hash: String,
    description: Option<String>,
}

impl MolecularProperty {
 pub fn new(
        molecule: &Molecule, 
        name: &str,
        value: serde_json::Value,
        units: Option<String>,
        quality: Option<String>,
        preferred: bool,
        description: Option<String>,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(molecule.inchikey().as_bytes());
        hasher.update(name.as_bytes());
        hasher.update(value.to_string().as_bytes());
        if let Some(u) = &units {
            hasher.update(u.as_bytes());
        }
        if let Some(q) = &quality {
            hasher.update(q.as_bytes());
        }
        let value_hash = format!("{:x}", hasher.finalize());
        MolecularProperty {
            id: Uuid::new_v4(),
            molecule: molecule.to_string(),
            name: name.to_string(),
            value,
            units,
            quality,
            preferred,
            value_hash,
            description,
        }
    }

    pub fn value_hash(&self) -> &str {
        &self.value_hash
    }
}
