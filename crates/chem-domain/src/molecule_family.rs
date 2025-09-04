use crate::molecule::Molecule;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use crate::error::DomainError;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeFamily {
    id: Uuid,
    name: Option<String>,
    description: Option<String>,
    family_hash: String,
    provenance: serde_json::Value,
    frozen: bool,
    molecules: Vec<Molecule>,
}
impl MoleculeFamily {
    /// Crea una familia inmutable a partir de moléculas con metadatos
    pub fn new<I>(mols: I, provenance: serde_json::Value) -> Result<Self, DomainError>
    where I: IntoIterator<Item = Molecule>
    {
        let list: Vec<Molecule> = mols.into_iter().collect();
        // Validar duplicados por InChIKey
        let mut seen = HashSet::new();
        for m in &list {
            if !seen.insert(m.inchikey().to_owned()) {
                return Err(DomainError::ValidationError(
                    format!("Molécula duplicada en familia: {}", m.inchikey())
                ));
            }
        }
        let id = Uuid::new_v4();
        // Hash solo basado en la secuencia de InChIKeys de las moléculas
        let mut hasher = Sha256::new();
        for m in &list {
            hasher.update(m.inchikey().as_bytes());
        }
        let family_hash = format!("{:x}", hasher.finalize());
        Ok(MoleculeFamily {
            id,
            name: None,
            description: None,
            family_hash,
            provenance,
            frozen: true,
            molecules: list,
        })
    } 
    pub fn with_name(&self, name: impl Into<String>) -> Self {
        let mut new = self.clone();
        new.name = Some(name.into());
        new
    }
    pub fn with_description(&self, desc: impl Into<String>) -> Self {
        let mut new = self.clone();
        new.description = Some(desc.into());
        new
    }
    pub fn add_molecule(&self, molecule: Molecule) -> Result<Self, DomainError> {
        // No permitir duplicados en la familia
        if self.molecules.iter().any(|m| m.inchikey() == molecule.inchikey()) {
            return Err(DomainError::ValidationError(
                format!("Molécula ya existe: {}", molecule.inchikey())
            ));
        }
        let mut list = self.molecules.clone();
        list.push(molecule);
        let id = Uuid::new_v4();
        let mut hasher = Sha256::new();
        for m in &list {
            hasher.update(m.inchikey().as_bytes());
        }
        let family_hash = format!("{:x}", hasher.finalize());
        Ok(MoleculeFamily {
            id,
            name: self.name.clone(),
            description: self.description.clone(),
            family_hash,
            provenance: self.provenance.clone(),
            frozen: true,
            molecules: list,
        })
    }
    /// Devuelve un nuevo objeto sin la molécula cuyo InChIKey coincide
    pub fn remove_molecule(&self, inchikey: &str) -> Self {
        let list: Vec<Molecule> = self.molecules.iter()
            .cloned()
            .filter(|m| m.inchikey() != inchikey)
            .collect();
        let id = Uuid::new_v4();
        // Hash = sha256(inchikeys concatenados post-remoción)
        let mut hasher = Sha256::new();
        for m in &list {
            hasher.update(m.inchikey().as_bytes());
        }
        let family_hash = format!("{:x}", hasher.finalize());
        MoleculeFamily {
            id,
            name: self.name.clone(),
            description: self.description.clone(),
            family_hash,
            provenance: self.provenance.clone(),
            frozen: true,
            molecules: list,
        }
    }
    /// Retorna la lista inmutable de moléculas de la familia
    pub fn molecules(&self) -> &[Molecule] {
        &self.molecules
    }

    pub fn family_hash(&self) -> &str {
        &self.family_hash
    }
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
    pub fn id(&self) -> Uuid { self.id }
    pub fn name(&self) -> Option<&String> { self.name.as_ref() }
    pub fn description(&self) -> Option<&String> { self.description.as_ref() }
    pub fn compare(&self, other: &MoleculeFamily) -> bool {
        self.family_hash == other.family_hash
    }
}
impl<'a> IntoIterator for &'a MoleculeFamily {
    type Item = &'a Molecule;
    type IntoIter = std::slice::Iter<'a, Molecule>;
    fn into_iter(self) -> Self::IntoIter {
        self.molecules.iter()
    }
}
// Permite consumir la familia y obtener un iterator de Molecule
impl IntoIterator for MoleculeFamily {
    type Item = Molecule;
    type IntoIter = std::vec::IntoIter<Molecule>;
    fn into_iter(self) -> Self::IntoIter {
        self.molecules.into_iter()
    }
}
