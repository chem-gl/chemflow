use std::collections::HashMap;

use crate::database::repository::WorkflowExecutionRepository;
use crate::providers::molecule::traitmolecule::MoleculeProvider;
use crate::providers::properties::trait_properties::PropertiesProvider;
use crate::workflow::step::{WorkflowStep, StepInput, StepOutput};
use crate::data::family::MoleculeFamily;
 
pub struct WorkflowManager {
    execution_repo: WorkflowExecutionRepository,
    molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
    properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
}

impl WorkflowManager {
    pub fn new(
        execution_repo: WorkflowExecutionRepository,
        molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
    ) -> Self {
        Self {
            execution_repo,
            molecule_providers,
            properties_providers,
        }
    }
    
    pub async fn execute_step(
        &mut self,
        step: &dyn WorkflowStep,
        input_families: Vec<MoleculeFamily>,
        step_parameters: HashMap<String, serde_json::Value>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        let input = StepInput {
            families: input_families,
            parameters: step_parameters,
        };
        
    let _ = step.get_id();
    let _ = step.get_name();
    let _ = step.get_description();
    let _ = step.get_required_input_types();
    let _ = step.get_output_types();
    let _ = step.allows_branching();
    let output = step.execute(
            input,
            &self.molecule_providers,
            &self.properties_providers,
        ).await?;
        
        // Guardar la ejecución del step en la base de datos
        self.execution_repo.save_step_execution(&output.execution_info).await?;
        
        Ok(output)
    }
    
  }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;
    use crate::workflow::step::{WorkflowStep, StepInput, StepOutput, StepStatus, StepExecutionInfo};
    use crate::providers::molecule::traitmolecule::MoleculeProvider;
    use crate::providers::properties::trait_properties::PropertiesProvider;
    use async_trait::async_trait;

    struct DummyStep;

    #[async_trait]
    impl WorkflowStep for DummyStep {
        fn get_id(&self) -> Uuid { Uuid::new_v4() }
        fn get_name(&self) -> &str { "dummy" }
        fn get_description(&self) -> &str { "dummy desc" }
        fn get_required_input_types(&self) -> Vec<String> { vec![] }
        fn get_output_types(&self) -> Vec<String> { vec![] }
        fn allows_branching(&self) -> bool { false }

    async fn execute(
            &self,
            input: StepInput,
            _m: &HashMap<String, Box<dyn MoleculeProvider>>,
            _p: &HashMap<String, Box<dyn PropertiesProvider>>,
        ) -> Result<StepOutput, Box<dyn std::error::Error>> {
            Ok(StepOutput {
                families: input.families.clone(),
                results: input.parameters.clone(),
                execution_info: StepExecutionInfo {
                    step_id: Uuid::new_v4(),
                    parameters: input.parameters.clone(),
                    providers_used: Vec::new(),
                    start_time: chrono::Utc::now(),
                    end_time: chrono::Utc::now(),
                    status: StepStatus::Completed,
                },
            })
        }
    }

    #[tokio::test]
    async fn test_execute_step_manager() {
        let repo = WorkflowExecutionRepository::new(true);
        let mol = HashMap::new();
        let props = HashMap::new();
        let mut manager = WorkflowManager::new(repo, mol, props);

        let dummy = DummyStep;
        let families: Vec<MoleculeFamily> = Vec::new();
        let mut params = HashMap::new();
        params.insert("key".to_string(), serde_json::json!("value"));

        let output = manager.execute_step(&dummy, families.clone(), params.clone())
            .await.expect("manager exec");
    assert_eq!(output.families.len(), families.len());
        assert_eq!(output.results.get("key").unwrap(), &serde_json::json!("value"));
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
    }
}