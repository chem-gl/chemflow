use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::Value;

use crate::providers::molecule::traitmolecule::{MoleculeProvider, ParameterDefinition, ParameterType};
use crate::data::family::{MoleculeFamily, ProviderReference};
use crate::molecule::Molecule;

/// Provides a mock family of antioxidant seed molecules.
pub struct AntioxidantSeedProvider;

#[async_trait]
impl MoleculeProvider for AntioxidantSeedProvider {
    fn get_name(&self) -> &str { "antiox_seed" }
    fn get_version(&self) -> &str { "0.1.0" }
    fn get_description(&self) -> &str { "Mock antioxidant reference molecules" }
    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
        let mut m = HashMap::new();
        m.insert("include_phenolics".into(), ParameterDefinition { name: "include_phenolics".into(), description: "Include phenolic seeds".into(), data_type: ParameterType::Boolean, required: false, default_value: Some(Value::Bool(true)) });
        m.insert("extra_seeds".into(), ParameterDefinition { name: "extra_seeds".into(), description: "Additional SMILES".into(), data_type: ParameterType::Array, required: false, default_value: Some(Value::Array(vec![])) });
        m
    }
    async fn get_molecule_family(&self, parameters: &HashMap<String, Value>) -> Result<MoleculeFamily, Box<dyn std::error::Error>> {
        let mut fam = MoleculeFamily::new("Antioxidant Seeds".into(), Some("Mocked reference antioxidant molecules".into()));
        let include_phenolics = parameters.get("include_phenolics").and_then(|v| v.as_bool()).unwrap_or(true);
        if include_phenolics {
            for smi in ["O=CC1=CC=CC(O)=C1O", "CC1=C(O)C=C(O)C=C1O", "C1=CC(=CC=C1O)O"] { // simple phenolic-like examples
                let mol = Molecule::from_smiles(smi.to_string())?;
                fam.molecules.push(mol);
            }
        }
        if let Some(Value::Array(extra)) = parameters.get("extra_seeds") {
            for v in extra { if let Some(smi) = v.as_str() { fam.molecules.push(Molecule::from_smiles(smi.to_string())?); } }
        }
        fam.provenance = Some(crate::data::family::FamilyProvenance {
            created_in_step: None,
            creation_provider: Some(ProviderReference {
                provider_type: "molecule".into(),
                provider_name: self.get_name().into(),
                provider_version: self.get_version().into(),
                execution_parameters: parameters.clone(),
                execution_id: uuid::Uuid::new_v4(),
            })
        });
        Ok(fam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_antioxidant_seed_provider_basic() {
        let prov = AntioxidantSeedProvider;
        let fam = prov.get_molecule_family(&HashMap::new()).await.unwrap();
        assert!(!fam.molecules.is_empty());
    }
}
