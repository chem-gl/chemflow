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
    /// Map property name -> property entry (values + provider metadata)
    pub properties: HashMap<String, FamilyProperty>,
    /// Arbitrary parameters associated to this family (frozen at creation / mutation events)
    pub parameters: HashMap<String, serde_json::Value>,
    /// Original provider that generated the base family (if any)
    pub source_provider: Option<ProviderReference>,
}

/// Represents a property (possibly multi-valued) attached to a family along with
/// the provider & parameters used to calculate it for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyProperty {
    pub values: Vec<LogPData>,
    pub provider: ProviderReference,
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

    /// Attach a property with complete traceability information.
    pub fn add_property(
        &mut self,
        property_name: impl Into<String>,
        data: Vec<LogPData>,
        provider_reference: ProviderReference,
    ) {
        let entry = FamilyProperty { values: data, provider: provider_reference };
        self.properties.insert(property_name.into(), entry);
    }

    pub fn get_property(&self, property_name: &str) -> Option<&FamilyProperty> {
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
        
    family.add_property("logp", logp_data.clone(), provider_ref.clone());
        
        // Test get_property
        let property = family.get_property("logp");
        assert!(property.is_some());
    let p = property.unwrap();
    assert_eq!(p.values.len(), 1);
    assert_eq!(p.values[0].value, 1.5);
    assert_eq!(p.provider.provider_name, "test_provider");
        
        let non_existent = family.get_property("nonexistent");
        assert!(non_existent.is_none());
    }
}