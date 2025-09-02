use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use crate::data::family::MoleculeFamily;
use crate::data::types::LogPData;
use crate::providers::properties::trait_properties::{ParameterDefinition, ParameterType, PropertiesProvider};
/// Mock provider generating antioxidant activity scores per molecule.
pub struct AntioxidantActivityPropertiesProvider;
#[async_trait]
impl PropertiesProvider for AntioxidantActivityPropertiesProvider {
    fn get_name(&self) -> &str {
        "antiox_activity"
    }
    fn get_version(&self) -> &str {
        "0.1.0"
    }
    fn get_description(&self) -> &str {
        "Mock antioxidant activity scorer"
    }
    fn get_supported_properties(&self) -> Vec<String> {
        vec!["radical_scavenging_score".into()]
    }
    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
        let mut m = HashMap::new();
        m.insert("temperature".into(),
                 ParameterDefinition { name: "temperature".into(),
                                       description: "Assay temperature".into(),
                                       data_type: ParameterType::Number,
                                       required: false,
                                       default_value: Some(Value::Number(25.into())) });
        m.insert("method".into(),
                 ParameterDefinition { name: "method".into(),
                                       description: "Method variant".into(),
                                       data_type: ParameterType::String,
                                       required: false,
                                       default_value: Some(Value::String("std".into())) });
        m
    }
    async fn calculate_properties(&self, family: &MoleculeFamily, _parameters: &HashMap<String, Value>) -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
        Ok(family.molecules
                 .iter()
                 .enumerate()
                 .map(|(i, m)| LogPData { value: 0.5 + 0.05 * (i as f64) + (m.smiles.len() as f64 * 0.01),
                                          source: "antiox_activity_mock".into(),
                                          frozen: false,
                                          timestamp: Utc::now() })
                 .collect())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_activity_calculation() {
        let provider = AntioxidantActivityPropertiesProvider;
        let mut fam = MoleculeFamily::new("F".into(), None);
        fam.molecules = vec![]; // no molecules -> empty score list
        let scores = provider.calculate_properties(&fam, &HashMap::new()).await.unwrap();
        assert_eq!(scores.len(), 0);
    }
}
