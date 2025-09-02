use async_trait::async_trait;
use chemflow_rust::data::family::{MoleculeFamily, ProviderReference};
use chemflow_rust::database::repository::WorkflowExecutionRepository;
use chemflow_rust::providers::data::trait_dataprovider::DataProvider;
use chemflow_rust::providers::molecule::traitmolecule::MoleculeProvider;
use chemflow_rust::providers::properties::trait_properties::PropertiesProvider;
use chemflow_rust::workflow::manager::WorkflowManager;
use chemflow_rust::workflow::step::{StepExecutionInfo, StepInput, StepOutput, StepStatus, WorkflowStep};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

struct PropStep {
    id: Uuid,
    name: &'static str,
}

#[async_trait]
impl WorkflowStep for PropStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    fn get_name(&self) -> &str {
        self.name
    }
    fn get_description(&self) -> &str {
        "prop step"
    }
    fn get_required_input_types(&self) -> Vec<String> {
        vec!["molecule_family".into()]
    }
    fn get_output_types(&self) -> Vec<String> {
        vec!["molecule_family".into()]
    }
    fn allows_branching(&self) -> bool {
        true
    }
    async fn execute(&self, mut input: StepInput, _m: &HashMap<String, Box<dyn MoleculeProvider>>, _p: &HashMap<String, Box<dyn PropertiesProvider>>, _d: &HashMap<String, Box<dyn DataProvider>>) -> Result<StepOutput, Box<dyn std::error::Error>> {
        // Add a dummy property contribution to each family
        for fam in &mut input.families {
            let provider_ref = ProviderReference { provider_type: "prop".into(),
                                                   provider_name: self.name.into(),
                                                   provider_version: "1.0".into(),
                                                   execution_parameters: HashMap::new(),
                                                   execution_id: self.id };
            fam.add_property("logp",
                             vec![chemflow_rust::data::types::LogPData { value: 1.0,
                                                                         source: self.name.into(),
                                                                         frozen: false,
                                                                         timestamp: Utc::now() }],
                             provider_ref,
                             Some(self.id));
        }
        Ok(StepOutput { families: input.families.clone(),
                        results: HashMap::new(),
                        execution_info: StepExecutionInfo { step_id: self.id,
                                                            parameters: input.parameters.clone(),
                                                            parameter_hash: Some(chemflow_rust::database::repository::compute_sorted_hash(&input.parameters)),
                                                            providers_used: Vec::new(),
                                                            start_time: Utc::now(),
                                                            end_time: Utc::now(),
                                                            status: StepStatus::Completed,
                                                            root_execution_id: Uuid::new_v4(),
                                                            parent_step_id: None,
                                                            branch_from_step_id: None,
                                                            input_family_ids: input.families.iter().map(|f| f.id).collect() } })
    }
}

#[tokio::test]
async fn test_multi_provider_and_autobranch() {
    // In-memory repo (not exercising DB writes here to keep test light)
    let repo = WorkflowExecutionRepository::new(true);
    let mut manager = WorkflowManager::new(repo, HashMap::new(), HashMap::new(), HashMap::new());

    // Create base family
    let mut fam = MoleculeFamily::new("F1".into(), None);
    fam.recompute_hash();
    let families = vec![fam];

    // First step with parameter X=1
    let s1 = PropStep { id: Uuid::new_v4(), name: "P1" };
    let mut params1 = HashMap::new();
    params1.insert("X".into(), serde_json::json!(1));
    let out1 = manager.execute_step(&s1, families.clone(), params1).await.unwrap();
    assert_eq!(out1.families[0].get_property("logp").unwrap().providers.len(), 1);

    // Second step same params -> should not branch
    let s2 = PropStep { id: Uuid::new_v4(), name: "P2" };
    let mut params2 = HashMap::new();
    params2.insert("X".into(), serde_json::json!(1));
    let out2 = manager.execute_step(&s2, out1.families.clone(), params2).await.unwrap();
    assert!(out2.execution_info.branch_from_step_id.is_none(), "No branch expected");

    // Third step changed params -> causes branch
    let s3 = PropStep { id: Uuid::new_v4(), name: "P3" };
    let mut params3 = HashMap::new();
    params3.insert("X".into(), serde_json::json!(2));
    let out3 = manager.execute_step(&s3, out2.families.clone(), params3).await.unwrap();
    assert!(out3.execution_info.branch_from_step_id.is_some(), "Branch expected due to param change");
    let prop = out3.families[0].get_property("logp").unwrap();
    assert_eq!(prop.providers.len(), 3, "Three provider contributions accumulated");
    assert_eq!(prop.originating_steps.len(), 3);
}
