use crate::{
    data::{family::MoleculeFamily, types::LogPData},
    providers::properties::trait_properties::{ParameterDefinition, ParameterType, PropertiesProvider},
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
/// Generic physchem provider that can mock multiple property kinds
pub struct GenericPhysChemProvider {
    name: String,
    version: String,
    supported: Vec<String>,
}
impl GenericPhysChemProvider {
    pub fn new() -> Self {
        Self { name: "generic_physchem".into(),
               version: "0.1.0".into(),
               supported: vec!["logp",
                               "logd",
                               "pka",
                               "logs",
                               "mw",
                               "psa",
                               "volume",
                               "homo_energy",
                               "lumo_energy",
                               "partial_charge",
                               "polarizability",
                               "rotor_count",
                               "mr",
                               "hydration_energy",
                               "ld50"].into_iter()
                                      .map(|s| s.to_string())
                                      .collect() }
    }
}
impl Default for GenericPhysChemProvider {
    fn default() -> Self {
        Self::new()
    }
}
#[async_trait]
impl PropertiesProvider for GenericPhysChemProvider {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_version(&self) -> &str {
        &self.version
    }
    fn get_description(&self) -> &str {
        "Mock multi-property physicochemical provider"
    }
    fn get_supported_properties(&self) -> Vec<String> {
        self.supported.clone()
    }
    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
        let mut m = HashMap::new();
        m.insert("seed".into(),
                 ParameterDefinition { name: "seed".into(),
                                       description: "Random seed".into(),
                                       data_type: ParameterType::Number,
                                       required: false,
                                       default_value: Some(Value::Number(42.into())) });
        m.insert("intensity".into(),
                 ParameterDefinition { name: "intensity".into(),
                                       description: "Scaling factor".into(),
                                       data_type: ParameterType::Number,
                                       required: false,
                                       default_value: Some(Value::Number(1.into())) });
        m
    }
    async fn calculate_properties(&self,
                                  _molecule_family: &MoleculeFamily,
                                  parameters: &HashMap<String, Value>)
                                  -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
        let scale = parameters.get("intensity").and_then(|v| v.as_f64()).unwrap_or(1.0);
        Ok(self.supported
               .iter()
               .enumerate()
               .map(|(i, _p)| LogPData { value: (i as f64 + 1.0) * scale,
                                         source: self.name.clone(),
                                         frozen: false,
                                         timestamp: Utc::now() })
               .collect())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::family::MoleculeFamily;
    use tokio;
    #[tokio::test]
    async fn test_generic_provider() {
        let prov = GenericPhysChemProvider::new();
        let fam = MoleculeFamily::new("f".into(), None);
        let vals = prov.calculate_properties(&fam, &HashMap::new()).await.unwrap();
        assert!(!vals.is_empty());
    }
}
