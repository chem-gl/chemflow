use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::data::family::MoleculeFamily;
use crate::molecule::Molecule;
use crate::providers::molecule::traitmolecule::{MoleculeProvider, ParameterDefinition, ParameterType};

pub struct TestMoleculeProvider;

impl TestMoleculeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestMoleculeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MoleculeProvider for TestMoleculeProvider {
    fn get_name(&self) -> &str {
        "Test Molecule Provider"
    }

    fn get_version(&self) -> &str {
        "1.0.0"
    }

    fn get_description(&self) -> &str {
        "Provides test molecules for development and testing"
    }

    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
        let mut params = HashMap::new();
        params.insert("count".to_string(),
                      ParameterDefinition { name: "count".to_string(),
                                            description: "Number of molecules to generate".to_string(),
                                            data_type: ParameterType::Number,
                                            required: false,
                                            default_value: Some(Value::Number(10.into())) });
        params
    }

    async fn get_molecule_family(&self, parameters: &HashMap<String, Value>) -> Result<MoleculeFamily, Box<dyn std::error::Error>> {
        let count = parameters.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let mut molecules = Vec::new();
        for i in 0..count {
            let smiles = format!("C{}", i);
            let inchi = format!("InChI=1S/C{}/c1-{}", i, i);
            let inchikey = format!("TESTKEY{}", i);
            molecules.push(Molecule::new(inchikey, smiles, inchi, Some(format!("Test Molecule {}", i))));
        }

        let mut family = MoleculeFamily::new(format!("Test Family with {} molecules", count), Some("Generated for testing".to_string()));
        family.molecules = molecules;
        Ok(family)
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::molecule::traitmolecule::MoleculeProvider;

    use super::TestMoleculeProvider;

    #[test]
    fn test_provider_metadata_and_parameters() {
        let provider = TestMoleculeProvider::new();
        assert_eq!(provider.get_name(), "Test Molecule Provider");
        assert_eq!(provider.get_version(), "1.0.0");
        assert_eq!(provider.get_description(), "Provides test molecules for development and testing");
        let params = provider.get_available_parameters();
        assert!(params.contains_key("count"));
        let def = &params["count"];
        assert_eq!(def.name, "count");
        assert_eq!(def.description, "Number of molecules to generate");
        assert!(!def.required);
        // default_value is Some(Value::Number(10))
        match &def.default_value {
            Some(v) => assert_eq!(v.as_u64().unwrap(), 10),
            None => panic!("default_value should be present"),
        }
    }
}
