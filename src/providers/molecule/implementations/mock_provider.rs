use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::Value;

use crate::data::family::{MoleculeFamily, ProviderReference};
use crate::molecule::Molecule;
use crate::providers::molecule::traitmolecule::MoleculeProvider;


pub struct MockMoleculeProvider {
    pub name: String,
    pub version: String,
}

impl MockMoleculeProvider {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }
}

#[async_trait]
impl MoleculeProvider for MockMoleculeProvider {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_version(&self) -> &str {
        &self.version
    }

    fn get_description(&self) -> &str {
        "Mock molecule provider for testing purposes"
    }

    fn get_available_parameters(&self) -> HashMap<String, crate::providers::molecule::traitmolecule::ParameterDefinition> {
        HashMap::new()
    }

    async fn get_molecule_family(
        &self,
        parameters: &HashMap<String, Value>,
    ) -> Result<MoleculeFamily, Box<dyn std::error::Error>> {
        let molecule = Molecule::from_smiles("CCO".to_string())?;
        let mut family = MoleculeFamily::new("Mock Family".to_string(), Some("Mock description".to_string()));
        family.molecules.push(molecule);

        let provider_ref = ProviderReference {
            provider_type: "molecule".to_string(),
            provider_name: self.name.clone(),
            provider_version: self.version.clone(),
            execution_parameters: parameters.clone(),
            execution_id: uuid::Uuid::new_v4(),
        };
        family.source_provider = Some(provider_ref);

        Ok(family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider_new() {
        let provider = MockMoleculeProvider::new("Test Name".to_string(), "1.0.0".to_string());
        assert_eq!(provider.name, "Test Name");
        assert_eq!(provider.version, "1.0.0");
    }
}
