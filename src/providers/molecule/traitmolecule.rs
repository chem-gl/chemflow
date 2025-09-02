use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::Value;

use crate::data::family::MoleculeFamily;

#[async_trait]
pub trait MoleculeProvider: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_version(&self) -> &str;
    fn get_description(&self) -> &str;
    fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition>;
    
    async fn get_molecule_family(
        &self,
        parameters: &HashMap<String, Value>,
    ) -> Result<MoleculeFamily, Box<dyn std::error::Error>>;
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

fn _use_molecule_params() {
    let pd = ParameterDefinition {
        name: String::new(),
        description: String::new(),
        data_type: ParameterType::String,
        required: false,
        default_value: None,
    };
    // Access all fields
    let _ = &pd.name;
    let _ = &pd.description;
    let _ = &pd.data_type;
    let _ = pd.required;
    let _ = &pd.default_value;
    // Use all variants
    let _ = ParameterType::String;
    let _ = ParameterType::Number;
    let _ = ParameterType::Boolean;
    let _ = ParameterType::Array;
    let _ = ParameterType::Object;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestMoleculeProvider;

    #[async_trait]
    impl MoleculeProvider for TestMoleculeProvider {
        fn get_name(&self) -> &str {
            "Test Provider"
        }
        
        fn get_version(&self) -> &str {
            "1.0.0"
        }
        
        fn get_description(&self) -> &str {
            "Test molecule provider"
        }
        
        fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
            let mut params = HashMap::new();
            params.insert("param1".to_string(), ParameterDefinition {
                name: "param1".to_string(),
                description: "Test parameter".to_string(),
                data_type: ParameterType::String,
                required: true,
                default_value: Some(Value::String("default".to_string())),
            });
            params
        }
        
        async fn get_molecule_family(
            &self,
            _parameters: &HashMap<String, Value>,
        ) -> Result<MoleculeFamily, Box<dyn std::error::Error>> {
            Ok(MoleculeFamily::new("Test".to_string(), None))
        }
    }

    #[test]
    fn test_molecule_provider_methods() {
        let provider = TestMoleculeProvider;
        
        // Call all methods
        assert_eq!(provider.get_name(), "Test Provider");
        assert_eq!(provider.get_version(), "1.0.0");
        assert_eq!(provider.get_description(), "Test molecule provider");
        
        let params = provider.get_available_parameters();
        assert!(params.contains_key("param1"));
    }

    #[test]
    fn test_parameter_definition_fields() {
        let param_def = ParameterDefinition {
            name: "test_param".to_string(),
            description: "Test description".to_string(),
            data_type: ParameterType::String,
            required: false,
            default_value: Some(Value::Bool(true)),
        };

        // Access all fields
        assert_eq!(param_def.name, "test_param");
        assert_eq!(param_def.description, "Test description");
        assert!(matches!(param_def.data_type, ParameterType::String));
        assert!(!param_def.required);
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
mod traitmolecule_usage_tests {
    use super::*;
    use std::collections::HashMap;

    struct DummyMoleculeProv;

    #[async_trait]
    impl MoleculeProvider for DummyMoleculeProv {
        fn get_name(&self) -> &str { "dummy" }
        fn get_version(&self) -> &str { "0.0" }
        fn get_description(&self) -> &str { "desc" }
        fn get_available_parameters(&self) -> HashMap<String, ParameterDefinition> {
            let mut m = HashMap::new();
            m.insert("x".to_string(), ParameterDefinition { name: "x".to_string(), description: "d".to_string(), data_type: ParameterType::Number, required: true, default_value: None });
            m
        }
        async fn get_molecule_family(
            &self,
            _parameters: &HashMap<String, serde_json::Value>
        ) -> Result<crate::data::family::MoleculeFamily, Box<dyn std::error::Error>> {
            Ok(crate::data::family::MoleculeFamily::new("n".to_string(), None))
        }
    }

    #[test]
    fn test_dummy_molecule_provider_methods() {
        let prov = DummyMoleculeProv;
        assert_eq!(prov.get_name(), "dummy");
        assert_eq!(prov.get_version(), "0.0");
        assert_eq!(prov.get_description(), "desc");
        let params = prov.get_available_parameters();
        assert!(params.contains_key("x"));
    }

    #[test]
    fn test_parameter_definition_and_types() {
        let pd = ParameterDefinition { name: "n".to_string(), description: "d".to_string(), data_type: ParameterType::Boolean, required: false, default_value: None };
        assert_eq!(pd.name, "n");
        assert_eq!(pd.description, "d");
        assert!(!pd.required);
        match pd.data_type {
            ParameterType::Boolean => {},
            _ => panic!(),
        }
        // Use all variants
        let _ = ParameterType::String;
        let _ = ParameterType::Number;
        let _ = ParameterType::Array;
        let _ = ParameterType::Object;
    }
}
