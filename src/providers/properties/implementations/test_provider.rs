use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::data::family::MoleculeFamily;
use crate::data::types::LogPData;
use crate::providers::properties::trait_properties::{PropertiesProvider, ParameterDefinition, ParameterType};

pub struct TestPropertiesProvider;

impl TestPropertiesProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PropertiesProvider for TestPropertiesProvider {
    fn get_name(&self) -> &str {
        "Test Properties Provider"
    }

    fn get_version(&self) -> &str {
        "1.0.0"
    }

    fn get_description(&self) -> &str {
        "Provides test properties for development and testing"
    }

    fn get_supported_properties(&self) -> Vec<String> {
        vec!["logp".to_string(), "molecular_weight".to_string()]
    }

    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
        let mut params = HashMap::new();
        params.insert("calculation_method".to_string(), ParameterDefinition {
            name: "calculation_method".to_string(),
            description: "Method to use for calculation".to_string(),
            data_type: ParameterType::String,
            required: false,
            default_value: Some(Value::String("test_method".to_string())),
        });
        params
    }

    async fn calculate_properties(
        &self,
        molecule_family: &MoleculeFamily,
        parameters: &HashMap<String, Value>
    ) -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
        let method = parameters.get("calculation_method")
            .and_then(|v| v.as_str())
            .unwrap_or("test_method");

        let mut results = Vec::new();
        for molecule in &molecule_family.molecules {
            let logp_value = molecule.smiles.len() as f64 * 0.1;
            let logp_data = LogPData {
                value: logp_value,
                source: format!("TestProvider_{}", method),
                frozen: false,
                timestamp: chrono::Utc::now(),
            };
            results.push(logp_data);
        }
        Ok(results)
    }
}
