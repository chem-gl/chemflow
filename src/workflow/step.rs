use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::data::family::{MoleculeFamily, ProviderReference};
use crate::providers::molecule::traitmolecule::{MoleculeProvider};
use crate::providers::properties::trait_properties::PropertiesProvider;
use crate::data::types::MolecularData;
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct StepInput {
    pub families: Vec<MoleculeFamily>,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepOutput {
    pub families: Vec<MoleculeFamily>,
    pub results: HashMap<String, serde_json::Value>,
    pub execution_info: StepExecutionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionInfo {
    pub step_id: Uuid,
    pub parameters: HashMap<String, serde_json::Value>,
    pub providers_used: Vec<ProviderReference>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub status: StepStatus,
    // Root execution flow id (constant for original workflow run)
    pub root_execution_id: Uuid,
    // Parent step id (previous step in linear flow) if any
    pub parent_step_id: Option<Uuid>,
    // If this execution is part of a branch, which step it branched from
    pub branch_from_step_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

#[async_trait]
pub trait WorkflowStep: Send + Sync {
    fn get_id(&self) -> Uuid;
    fn get_name(&self) -> &str;
    fn get_description(&self) -> &str;
    fn get_required_input_types(&self) -> Vec<String>;
    fn get_output_types(&self) -> Vec<String>;
    fn allows_branching(&self) -> bool;
    
    async fn execute(
        &self,
        input: StepInput,
        molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>>;
}

// ---- Parameter validation helpers ----
fn validate_parameters(
    provided: &HashMap<String, Value>,
    definitions: &HashMap<String, crate::providers::molecule::traitmolecule::ParameterDefinition>,
) -> Result<HashMap<String, Value>, String> {
    let mut result = provided.clone();
    for (k, def) in definitions {
        if !result.contains_key(k) {
            if def.required {
                return Err(format!("Missing required parameter: {k}"));
            }
            if let Some(default) = &def.default_value {
                result.insert(k.clone(), default.clone());
            }
        }
    }
    Ok(result)
}

fn validate_prop_parameters(
    provided: &HashMap<String, Value>,
    definitions: &HashMap<String, crate::providers::properties::trait_properties::ParameterDefinition>,
) -> Result<HashMap<String, Value>, String> {
    let mut result = provided.clone();
    for (k, def) in definitions {
        if !result.contains_key(k) {
            if def.required {
                return Err(format!("Missing required parameter: {k}"));
            }
            if let Some(default) = &def.default_value {
                result.insert(k.clone(), default.clone());
            }
        }
    }
    Ok(result)
}

// Implementaciones concretas de steps
pub struct MoleculeAcquisitionStep {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub provider_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl WorkflowStep for MoleculeAcquisitionStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_description(&self) -> &str {
        &self.description
    }
    
    fn get_required_input_types(&self) -> Vec<String> {
        Vec::new() // No requiere input
    }
    
    fn get_output_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn allows_branching(&self) -> bool {
        true
    }
    
    async fn execute(
        &self,
        _input: StepInput,
        molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        _properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        let provider = molecule_providers.get(&self.provider_name)
            .ok_or_else(|| format!("Provider {} not found", self.provider_name))?;
         let _ = provider.get_name();
        let _ = provider.get_version();
        let _ = provider.get_description();
            let mol_params = provider.get_available_parameters();
            for pd in mol_params.values() {
                let _ = &pd.name;
                let _ = &pd.description;
                let _ = &pd.data_type;
                let _ = &pd.required;
                let _ = &pd.default_value;
            }
  
        let param_defs = provider.get_available_parameters();
        let validated = validate_parameters(&self.parameters, &param_defs)
            .map_err(|e| format!("Parameter validation failed: {e}"))?;
        let family = provider.get_molecule_family(&validated).await?;
        
        Ok(StepOutput {
            families: vec![family],
            results: HashMap::new(),
            execution_info: StepExecutionInfo {
                step_id: self.id,
                parameters: validated.clone(),
                providers_used: vec![ProviderReference {
                    provider_type: "molecule".to_string(),
                    provider_name: self.provider_name.clone(),
                    provider_version: provider.get_version().to_string(),
                    execution_parameters: self.parameters.clone(),
                    execution_id: Uuid::new_v4(),
                }],
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                status: StepStatus::Completed,
                root_execution_id: Uuid::new_v4(),
                parent_step_id: None,
                branch_from_step_id: None,
            },
        })
    }
}

pub struct PropertiesCalculationStep {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub provider_name: String,
    pub property_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl WorkflowStep for PropertiesCalculationStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_description(&self) -> &str {
        &self.description
    }
    
    fn get_required_input_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn get_output_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn allows_branching(&self) -> bool {
        true
    }
    
    async fn execute(
        &self,
        input: StepInput,
        _molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        let provider = properties_providers.get(&self.provider_name)
            .ok_or_else(|| format!("Provider {} not found", self.provider_name))?;
         let _ = provider.get_name();
        let _ = provider.get_version();
        let _ = provider.get_description();
            let _ = provider.get_supported_properties();
            let prop_params = provider.get_available_parameters();
            for pd in prop_params.values() {
                let _ = &pd.name;
                let _ = &pd.description;
                let _ = &pd.data_type;
                let _ = &pd.required;
                let _ = &pd.default_value;
            }
 
        
        let param_defs = provider.get_available_parameters();
        let validated = validate_prop_parameters(&self.parameters, &param_defs)
            .map_err(|e| format!("Parameter validation failed: {e}"))?;
        let mut output_families = input.families.clone();
        for family in &mut output_families {
            let properties = provider.calculate_properties(family, &validated).await?;
            for data in &properties {
                let _ = data.get_value();
                let _ = data.get_source();
                let _ = data.is_frozen();
            }
            let _ = family.get_property(&self.property_name);
            family.add_property(self.property_name.clone(), properties, ProviderReference {
                provider_type: "properties".to_string(),
                provider_name: self.provider_name.clone(),
                provider_version: provider.get_version().to_string(),
                execution_parameters: self.parameters.clone(),
                execution_id: Uuid::new_v4(),
            });
        }
        
        Ok(StepOutput {
            families: output_families,
            results: HashMap::new(),
            execution_info: StepExecutionInfo {
                step_id: self.id,
                parameters: validated.clone(),
                providers_used: vec![ProviderReference {
                    provider_type: "properties".to_string(),
                    provider_name: self.provider_name.clone(),
                    provider_version: provider.get_version().to_string(),
                    execution_parameters: self.parameters.clone(),
                    execution_id: Uuid::new_v4(),
                }],
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                status: StepStatus::Completed,
                root_execution_id: Uuid::new_v4(),
                parent_step_id: None,
                branch_from_step_id: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::molecule::Molecule;
    use crate::data::family::MoleculeFamily;
    use crate::providers::molecule::implementations::test_provider::TestMoleculeProvider;
    use crate::providers::properties::implementations::test_provider::TestPropertiesProvider;

    struct TestWorkflowStep {
        id: Uuid,
        name: String,
        description: String,
    }

    #[async_trait]
    impl WorkflowStep for TestWorkflowStep {
        fn get_id(&self) -> Uuid {
            self.id
        }
        fn get_name(&self) -> &str {
            &self.name
        }
        fn get_description(&self) -> &str {
            &self.description
        }
        fn get_required_input_types(&self) -> Vec<String> {
            vec!["test_input".to_string()]
        }
        fn get_output_types(&self) -> Vec<String> {
            vec!["test_output".to_string()]
        }
        fn allows_branching(&self) -> bool {
            true
        }
        async fn execute(
            &self,
            _input: StepInput,
            _molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
            _properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
        ) -> Result<StepOutput, Box<dyn std::error::Error>> {
            Ok(StepOutput {
                families: Vec::new(),
                results: HashMap::new(),
                execution_info: StepExecutionInfo {
                    step_id: self.id,
                    parameters: HashMap::new(),
                    providers_used: Vec::new(),
                    start_time: chrono::Utc::now(),
                    end_time: chrono::Utc::now(),
                    status: StepStatus::Completed,
                    root_execution_id: Uuid::new_v4(),
                    parent_step_id: None,
                    branch_from_step_id: None,
                },
            })
        }
    }

   
    #[test]
    fn test_workflow_step_methods() {
        let step = TestWorkflowStep {
            id: Uuid::new_v4(),
            name: "Test Step".to_string(),
            description: "Test Description".to_string(),
        };

        assert_eq!(step.get_name(), "Test Step");
        assert_eq!(step.get_description(), "Test Description");
        assert_eq!(step.get_required_input_types(), vec!["test_input".to_string()]);
        assert_eq!(step.get_output_types(), vec!["test_output".to_string()]);
        assert!(step.allows_branching());
    
    }

    #[tokio::test]
    async fn test_molecule_acquisition_step_execute() {
        let mut mol_providers = HashMap::new();
        mol_providers.insert(
            "test_molecule".to_string(),
            Box::new(TestMoleculeProvider::new()) as Box<dyn MoleculeProvider>
        );
        let props_providers: HashMap<String, Box<dyn PropertiesProvider>> = HashMap::new();
        // Create step
        let step = MoleculeAcquisitionStep {
            id: Uuid::new_v4(),
            name: "Acquire".to_string(),
            description: "Acquire molecules".to_string(),
            provider_name: "test_molecule".to_string(),
            parameters: HashMap::new(),
        };
        // Execute
        let input = StepInput { families: Vec::new(), parameters: HashMap::new() };
        let output = step.execute(input, &mol_providers, &props_providers)
            .await.expect("execution should succeed");
        // Assertions
        assert_eq!(output.families.len(), 1);
        let family = &output.families[0];
        assert_eq!(family.molecules.len(), 10);
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
        assert_eq!(output.execution_info.providers_used.len(), 1);
        let prov_ref = &output.execution_info.providers_used[0];
        assert_eq!(prov_ref.provider_type, "molecule");
        assert_eq!(prov_ref.provider_name, "test_molecule");
    }

    #[tokio::test]
    async fn test_properties_calculation_step_execute() {
        // Setup provider
        let mol_providers: HashMap<String, Box<dyn MoleculeProvider>> = HashMap::new();
        let mut props_providers = HashMap::new();
        props_providers.insert(
            "test_properties".to_string(),
            Box::new(TestPropertiesProvider::new()) as Box<dyn PropertiesProvider>
        );
        // Prepare input family
        let mut family = MoleculeFamily::new("fam".to_string(), None);
        family.molecules.push(Molecule::new(
            "K".to_string(), "CC".to_string(), "I".to_string(), None
        ));
        let input = StepInput {
            families: vec![family.clone()],
            parameters: HashMap::new(),
        };
        // Create step
        let step = PropertiesCalculationStep {
            id: Uuid::new_v4(),
            name: "Calc".to_string(),
            description: "Calculate properties".to_string(),
            provider_name: "test_properties".to_string(),
            property_name: "logp".to_string(),
            parameters: HashMap::new(),
        };
        // Execute
        let output = step.execute(input, &mol_providers, &props_providers)
            .await.expect("execution should succeed");
        // Assertions
        assert_eq!(output.families.len(), 1);
    let out_family = &output.families[0];
    // After execution, property 'logp' should be present
    let prop = out_family.get_property("logp");
    assert!(prop.is_some());
    assert!(!prop.unwrap().values.is_empty());
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
        let prov_ref = &output.execution_info.providers_used[0];
        assert_eq!(prov_ref.provider_type, "properties");
        assert_eq!(prov_ref.provider_name, "test_properties");
    }
}