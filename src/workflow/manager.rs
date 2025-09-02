use std::collections::HashMap;

use crate::database::repository::WorkflowExecutionRepository;
use crate::providers::molecule::traitmolecule::MoleculeProvider;
use crate::providers::properties::trait_properties::PropertiesProvider;
use crate::workflow::step::{WorkflowStep, StepInput, StepOutput};
use crate::providers::data::trait_dataprovider::DataProvider;
use crate::data::family::MoleculeFamily;
 
pub struct WorkflowManager {
    execution_repo: WorkflowExecutionRepository,
    molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
    properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
    data_providers: HashMap<String, Box<dyn DataProvider>>,
    current_root_execution_id: uuid::Uuid,
    last_step_id: Option<uuid::Uuid>,
    branch_origin: Option<uuid::Uuid>,
}

impl WorkflowManager {
    pub fn new(
        execution_repo: WorkflowExecutionRepository,
        molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
        data_providers: HashMap<String, Box<dyn DataProvider>>,
    ) -> Self {
        Self {
            execution_repo,
            molecule_providers,
            properties_providers,
            data_providers,
            current_root_execution_id: uuid::Uuid::new_v4(),
            last_step_id: None,
            branch_origin: None,
        }
    }

    pub fn root_execution_id(&self) -> uuid::Uuid { self.current_root_execution_id }
    pub fn last_step_id(&self) -> Option<uuid::Uuid> { self.last_step_id }
    pub fn repository(&self) -> &WorkflowExecutionRepository { &self.execution_repo }

    /// Starts a new independent workflow root execution context.
    pub fn start_new_flow(&mut self) -> uuid::Uuid {
        self.current_root_execution_id = uuid::Uuid::new_v4();
        self.last_step_id = None;
    self.branch_origin = None;
        self.current_root_execution_id
    }

    /// Creates a branch from a given previous step id: resets parent pointer but keeps root id.
    pub fn create_branch(&mut self, from_step_id: uuid::Uuid) -> uuid::Uuid {
    // Keep same root, mark branch origin so subsequent executions record it
    self.last_step_id = Some(from_step_id);
    self.branch_origin = Some(from_step_id);
        self.current_root_execution_id
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

        // Touch data providers to mark usage (prepares for future DataAggregationStep)
        for (k, prov) in &self.data_providers {
            let _ = k;
            let _ = prov.get_name();
            let _ = prov.get_version();
            let _ = prov.get_description();
            let _ = prov.get_available_parameters();
        }
        
    let _ = step.get_id();
    let _ = step.get_name();
    let _ = step.get_description();
    let _ = step.get_required_input_types();
    let _ = step.get_output_types();
    let _ = step.allows_branching();
    let mut output = step.execute(
            input,
            &self.molecule_providers,
            &self.properties_providers,
        ).await?;
        
        // Augment execution info with root / parent / branch metadata
        let mut exec = output.execution_info.clone();
        exec.root_execution_id = self.current_root_execution_id;
        exec.parent_step_id = self.last_step_id;
    exec.branch_from_step_id = self.branch_origin;
        self.last_step_id = Some(exec.step_id);

    self.execution_repo.save_step_execution(&exec).await?;
    // Reflect enriched metadata back into returned output
    output.execution_info = exec.clone();

        // Persist families and relationships
        for fam in &output.families {
            let _ = self.execution_repo.upsert_family(fam).await; // ignore errors for now
            let _ = self.execution_repo.link_step_family(exec.step_id, fam.id).await;
        }
        
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
                    root_execution_id: Uuid::new_v4(),
                    parent_step_id: None,
                    branch_from_step_id: None,
                },
            })
        }
    }

    #[tokio::test]
    async fn test_execute_step_manager() {
        let repo = WorkflowExecutionRepository::new(true);
        let mol = HashMap::new();
        let props = HashMap::new();
    let data = HashMap::new();
    let mut manager = WorkflowManager::new(repo, mol, props, data);

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

    #[tokio::test]
    async fn test_branching_methods_usage() {
        let repo = WorkflowExecutionRepository::new(true);
    let mut manager = WorkflowManager::new(repo, HashMap::new(), HashMap::new(), HashMap::new());
        let original_root = manager.current_root_execution_id;
        let new_root = manager.start_new_flow();
        assert_ne!(original_root, new_root);
        // Create a dummy previous step id and branch from it
        let prev = Uuid::new_v4();
        let same_root = manager.create_branch(prev);
        assert_eq!(same_root, manager.current_root_execution_id);
        assert_eq!(manager.last_step_id, Some(prev));
    }
}