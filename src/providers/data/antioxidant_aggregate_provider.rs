use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use crate::data::family::MoleculeFamily;
use crate::providers::data::trait_dataprovider::{DataParameterDefinition, DataParameterType, DataProvider};
/// Aggregates antioxidant activity statistics across families.
pub struct AntioxidantAggregateProvider;
#[async_trait]
impl DataProvider for AntioxidantAggregateProvider {
    fn get_name(&self) -> &str {
        "antiox_aggregate"
    }
    fn get_version(&self) -> &str {
        "0.1.0"
    }
    fn get_description(&self) -> &str {
        "Aggregate antioxidant stats"
    }
    fn get_available_parameters(&self) -> HashMap<String, DataParameterDefinition> {
        let mut m = HashMap::new();
        m.insert("min_score".into(),
                 DataParameterDefinition { name: "min_score".into(),
                                           description: "Minimum score filter".into(),
                                           data_type: DataParameterType::Number,
                                           required: false,
                                           default_value: Some(Value::Number(0.into())) });
        m
    }
    async fn calculate(&self, families: &[MoleculeFamily], params: &HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error>> {
        let min = params.get("min_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut values = Vec::new();
        for fam in families {
            if let Some(prop) = fam.get_property("radical_scavenging_score") {
                for v in &prop.values {
                    if v.value >= min {
                        values.push(v.value);
                    }
                }
            }
        }
        let count = values.len() as f64;
        let sum: f64 = values.iter().sum();
        let mean = if count > 0.0 { sum / count } else { 0.0 };
        Ok(serde_json::json!({ "count": count as i64, "mean": mean }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::family::{FamilyProperty, MoleculeFamily, ProviderReference};
    use crate::data::types::LogPData;
    use chrono::Utc;
    #[tokio::test]
    async fn test_aggregate_provider() {
        let _p = AntioxidantAggregateProvider;
        let mut fam = MoleculeFamily::new("F".into(), None);
        let _provider_ref = ProviderReference { provider_type: "properties".into(),
                                                provider_name: "mock".into(),
                                                provider_version: "0.0".into(),
                                                execution_parameters: HashMap::new(),
                                                execution_id: uuid::Uuid::new_v4() };
        fam.properties.insert("radical_scavenging_score".into(),
                              FamilyProperty { values: vec![LogPData { value: 1.0,
                                                                       source: "s".into(),
                                                                       frozen: false,
                                                                       timestamp: Utc::now() }],
                                               providers: Vec::new(),
                                               originating_steps: Vec::new() });
        let v = _p.calculate(&[fam], &HashMap::new()).await.unwrap();
        assert!(v.get("mean").is_some());
    }
}
