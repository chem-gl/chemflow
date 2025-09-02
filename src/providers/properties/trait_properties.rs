//! Trait y tipos para proveedores de propiedades químicas.
//! Proporciona el contrato para calcular propiedades sobre familias completas,
//! devolviendo vectores de datos (ej. LogP) con información temporal y de
//! fuente. La trazabilidad se logra registrando proveedor, versión y parámetros
//! en cada `FamilyProperty` cuando el step correspondiente invoca al provider.
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::data::family::MoleculeFamily;
use crate::data::types::LogPData;

#[async_trait]
pub trait PropertiesProvider: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_version(&self) -> &str;
    fn get_description(&self) -> &str;
    fn get_supported_properties(&self) -> Vec<String>;
    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition>;

    async fn calculate_properties(&self, molecule_family: &MoleculeFamily, parameters: &HashMap<String, Value>) -> Result<Vec<LogPData>, Box<dyn std::error::Error>>;
}
#[cfg(test)]
mod more_properties_provider_tests {
    use chrono::Utc;

    use super::*;

    struct AdvancedProvider;

    #[async_trait]
    impl PropertiesProvider for AdvancedProvider {
        fn get_name(&self) -> &str {
            "AdvancedProvider"
        }
        fn get_version(&self) -> &str {
            "2.1.0"
        }
        fn get_description(&self) -> &str {
            "Advanced provider for testing"
        }
        fn get_supported_properties(&self) -> Vec<String> {
            vec!["logp".to_string(), "solubility".to_string()]
        }
        fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
            let mut params = HashMap::new();
            params.insert("temperature".to_string(),
                          ParameterDefinition { name: "temperature".to_string(),
                                                description: "Temperature in Celsius".to_string(),
                                                data_type: ParameterType::Number,
                                                required: true,
                                                default_value: Some(Value::Number(serde_json::Number::from(25))) });
            params.insert("method".to_string(),
                          ParameterDefinition { name: "method".to_string(),
                                                description: "Calculation method".to_string(),
                                                data_type: ParameterType::String,
                                                required: false,
                                                default_value: Some(Value::String("default".to_string())) });
            params
        }
        async fn calculate_properties(&self, _molecule_family: &MoleculeFamily, _parameters: &HashMap<String, Value>) -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
            // Return two data points: one frozen and one not, all with source "advanced"
            Ok(vec![LogPData { value: 1.2,
                               source: "advanced".to_string(),
                               frozen: false,
                               timestamp: Utc::now() },
                    LogPData { value: 1.3,
                               source: "advanced".to_string(),
                               frozen: true,
                               timestamp: Utc::now() },])
        }
    }

    #[test]
    fn test_advanced_provider_metadata() {
        let provider = AdvancedProvider;
        assert_eq!(provider.get_name(), "AdvancedProvider");
        assert_eq!(provider.get_version(), "2.1.0");
        assert_eq!(provider.get_description(), "Advanced provider for testing");
        let props = provider.get_supported_properties();
        assert!(props.contains(&"logp".to_string()));
        assert!(props.contains(&"solubility".to_string()));
    }

    #[test]
    fn test_advanced_provider_parameters() {
        let provider = AdvancedProvider;
        let params = provider.get_available_parameters();
        assert!(params.contains_key("temperature"));
        assert!(params.contains_key("method"));
        let temp_param = params.get("temperature").unwrap();
        assert_eq!(temp_param.name, "temperature");
        assert_eq!(temp_param.description, "Temperature in Celsius");
        assert!(matches!(temp_param.data_type, ParameterType::Number));
        assert!(temp_param.required);
        assert_eq!(temp_param.default_value, Some(Value::Number(serde_json::Number::from(25))));
        let method_param = params.get("method").unwrap();
        assert_eq!(method_param.name, "method");
        assert_eq!(method_param.description, "Calculation method");
        assert!(matches!(method_param.data_type, ParameterType::String));
        assert!(!method_param.required);
        assert_eq!(method_param.default_value, Some(Value::String("default".to_string())));
    }

    #[tokio::test]
    async fn test_advanced_provider_calculate_properties() {
        let provider = AdvancedProvider;
        let molecule_family = MoleculeFamily::new("TestFamily".to_string(), None);
        let mut parameters = HashMap::new();
        parameters.insert("temperature".to_string(), Value::Number(serde_json::Number::from(30)));
        parameters.insert("method".to_string(), Value::String("fast".to_string()));
        let result = provider.calculate_properties(&molecule_family, &parameters).await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|d| d.frozen));
        assert!(result.iter().any(|d| !d.frozen));
        assert!(result.iter().all(|d| d.source == "advanced"));
    }
}
#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    pub name: String,
    pub description: String,
    pub data_type: ParameterType,
    pub required: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

fn _use_properties_params() {
    let pd = ParameterDefinition { name: String::new(),
                                   description: String::new(),
                                   data_type: ParameterType::String,
                                   required: false,
                                   default_value: None };
    let _ = &pd.name;
    let _ = &pd.description;
    let _ = &pd.data_type;
    let _ = pd.required;
    let _ = &pd.default_value;
    let _ = ParameterType::String;
    let _ = ParameterType::Number;
    let _ = ParameterType::Boolean;
    let _ = ParameterType::Array;
    let _ = ParameterType::Object;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::types::LogPData;
    use async_trait::async_trait;
    use chrono::Utc;

    struct TestPropertiesProvider;

    #[async_trait]
    impl PropertiesProvider for TestPropertiesProvider {
        fn get_name(&self) -> &str {
            "Test Properties Provider"
        }

        fn get_version(&self) -> &str {
            "1.0.0"
        }

        fn get_description(&self) -> &str {
            "Test properties provider"
        }

        fn get_supported_properties(&self) -> Vec<String> {
            vec!["logp".to_string()]
        }

        fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
            let mut params = HashMap::new();
            params.insert("param1".to_string(),
                          ParameterDefinition { name: "param1".to_string(),
                                                description: "Test parameter".to_string(),
                                                data_type: ParameterType::Number,
                                                required: false,
                                                default_value: Some(Value::Number(serde_json::Number::from_f64(1.0).unwrap())) });
            params
        }

        async fn calculate_properties(&self, _molecule_family: &MoleculeFamily, _parameters: &HashMap<String, Value>) -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
            Ok(vec![LogPData { value: 2.0,
                               source: "test".to_string(),
                               frozen: false,
                               timestamp: Utc::now() }])
        }
    }

    #[test]
    fn test_properties_provider_methods() {
        let provider = TestPropertiesProvider;

        // Call all methods
        assert_eq!(provider.get_name(), "Test Properties Provider");
        assert_eq!(provider.get_version(), "1.0.0");
        assert_eq!(provider.get_description(), "Test properties provider");
        assert_eq!(provider.get_supported_properties(), vec!["logp".to_string()]);

        let params = provider.get_available_parameters();
        assert!(params.contains_key("param1"));
    }

    #[test]
    fn test_parameter_definition_fields() {
        let param_def = ParameterDefinition { name: "test_param".to_string(),
                                              description: "Test description".to_string(),
                                              data_type: ParameterType::Boolean,
                                              required: true,
                                              default_value: Some(Value::Array(vec![])) };

        // Access all fields
        assert_eq!(param_def.name, "test_param");
        assert_eq!(param_def.description, "Test description");
        assert!(matches!(param_def.data_type, ParameterType::Boolean));
        assert!(param_def.required);
        assert!(param_def.default_value.is_some());
    }

    #[test]
    fn test_parameter_type_variants() {
        // Use all variants
        let _string_type = ParameterType::String;
        let _number_type = ParameterType::Number;
        let _boolean_type = ParameterType::Boolean;
        let _array_type = ParameterType::Array;
        let _object_type = ParameterType::Object;
    }
}

#[cfg(test)]
mod trait_properties_usage_tests {
    use super::*;
    use crate::data::types::LogPData;
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct DummyProvider;

    #[async_trait]
    impl PropertiesProvider for DummyProvider {
        fn get_name(&self) -> &str {
            "dummy"
        }
        fn get_version(&self) -> &str {
            "0.0"
        }
        fn get_description(&self) -> &str {
            "desc"
        }
        fn get_supported_properties(&self) -> Vec<String> {
            vec!["p".into()]
        }
        fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
            HashMap::new()
        }
        async fn calculate_properties(&self, _molecule_family: &crate::data::family::MoleculeFamily, _parameters: &HashMap<String, serde_json::Value>) -> Result<Vec<LogPData>, Box<dyn std::error::Error>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_dummy_provider_methods() {
        let prov = DummyProvider;
        assert_eq!(prov.get_name(), "dummy");
        assert_eq!(prov.get_version(), "0.0");
        assert_eq!(prov.get_description(), "desc");
        assert_eq!(prov.get_supported_properties(), vec!["p".to_string()]);
        let _params = prov.get_available_parameters();
        let mf = crate::data::family::MoleculeFamily::new("n".to_string(), None);
        let _ = prov.calculate_properties(&mf, &HashMap::new()).await;
    }
}
