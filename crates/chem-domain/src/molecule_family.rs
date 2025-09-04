use serde::{Serialize, Deserialize};
use uuid::Uuid;
use sha2::{Sha256, Digest};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeFamily {
    id: Uuid,
    ordered_keys: Vec<String>,
    family_hash: String,
    provenance: serde_json::Value,
    frozen: bool,
}

impl MoleculeFamily {
    pub fn from_iter<I>(keys: I, provenance: serde_json::Value) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut ordered: Vec<String> = keys.into_iter().map(|k| k.as_ref().to_string()).collect();
        ordered.sort();
        let mut hasher = Sha256::new();
        for key in &ordered {
            hasher.update(key.as_bytes());
        }
        let family_hash = format!("{:x}", hasher.finalize());
        MoleculeFamily {
            id: Uuid::new_v4(),
            ordered_keys: ordered,
            family_hash,
            provenance,
            frozen: true,
        }
    }
    pub fn family_hash(&self) -> &str {
        &self.family_hash
    }
    pub fn ordered_keys(&self) -> &[String] {
        &self.ordered_keys
    }
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
}
