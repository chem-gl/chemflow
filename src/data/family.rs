use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::molecule::Molecule;
use crate::data::types::LogPData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeFamily {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub molecules: Vec<Molecule>,
    pub properties: HashMap<String, Vec<LogPData>>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub source_provider: Option<ProviderReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReference {
    pub provider_type: String,
    pub provider_name: String,
    pub provider_version: String,
    pub execution_parameters: HashMap<String, serde_json::Value>,
    pub execution_id: Uuid,
}

impl MoleculeFamily {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            molecules: Vec::new(),
            properties: HashMap::new(),
            parameters: HashMap::new(),
            source_provider: None,
        }
    }

    pub fn add_property(
        &mut self,
        property_name: String,
        data: Vec<LogPData>,
        provider_reference: ProviderReference,
    ) {
        self.properties.insert(property_name, data);
        self.source_provider = Some(provider_reference);
    }

    pub fn get_property(&self, property_name: &str) -> Option<&Vec<LogPData>> {
        self.properties.get(property_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::types::LogPData;
    use chrono::Utc;

    #[test]
    fn test_get_property() {
        let mut family = MoleculeFamily::new("Test Family".to_string(), Some("Test".to_string()));
        
        let logp_data = vec![LogPData {
            value: 1.5,
            source: "test".to_string(),
            frozen: false,
            timestamp: Utc::now(),
        }];
        
        let provider_ref = ProviderReference {
            provider_type: "test".to_string(),
            provider_name: "test_provider".to_string(),
            provider_version: "1.0".to_string(),
            execution_parameters: HashMap::new(),
            execution_id: Uuid::new_v4(),
        };
        
        family.add_property("logp".to_string(), logp_data, provider_ref);
        
        // Test get_property
        let property = family.get_property("logp");
        assert!(property.is_some());
        assert_eq!(property.unwrap().len(), 1);
        
        let non_existent = family.get_property("nonexistent");
        assert!(non_existent.is_none());
    }
}