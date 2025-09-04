use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use crate::Molecule;

#[derive(Debug, Clone)]
pub struct MolecularProperty<'a, V, TypeMeta> {
    id: Uuid,
    molecule: &'a Molecule,
    property_type: String,
    value: V,
    quality: Option<String>,
    preferred: bool,
    value_hash: String, 
    metadata: TypeMeta,
}

impl<'a, V, TypeMeta> MolecularProperty<'a, V, TypeMeta>
where
    V: Serialize + Clone,
    TypeMeta: Serialize + Clone,
{
    pub fn new(
        molecule: &'a Molecule,
        property_type: &str,
        value: V,
        quality: Option<String>,
        preferred: bool,
        metadata: TypeMeta,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(molecule.inchikey().as_bytes());
        hasher.update(property_type.as_bytes());
        let value_json = serde_json::to_string(&value).unwrap_or_default();
        hasher.update(value_json.as_bytes());
        let metadata_json = serde_json::to_string(&metadata).unwrap_or_default();
        hasher.update(metadata_json.as_bytes());
        let value_hash = format!("{:x}", hasher.finalize());
        MolecularProperty {
            id: Uuid::new_v4(),
            molecule,
            property_type: property_type.to_string(),
            value,
            quality,
            preferred,
            value_hash,
            metadata,
        }
    }

    pub fn value_hash(&self) -> &str {
        &self.value_hash
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn molecule(&self) -> &Molecule {
        self.molecule
    }

    pub fn property_type(&self) -> &str {
        &self.property_type
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn quality(&self) -> Option<&String> {
        self.quality.as_ref()
    }

    pub fn preferred(&self) -> bool {
        self.preferred
    }

    pub fn metadata(&self) -> &TypeMeta {
        &self.metadata
    }

    pub fn with_quality(&self, quality: Option<String>) -> Self {
        MolecularProperty::new(
            self.molecule,
            &self.property_type,
            self.value.clone(),
            quality,
            self.preferred,
            self.metadata.clone()
        )
    }

    pub fn with_metadata(&self, metadata: TypeMeta) -> Self {
        MolecularProperty::new(
            self.molecule,
            &self.property_type,
            self.value.clone(),
            self.quality.clone(),
            self.preferred,
            metadata,
        )
    }
    pub fn compare(&self, other: &MolecularProperty<'a, V, TypeMeta>) -> bool {
        self.value_hash == other.value_hash
    }
}