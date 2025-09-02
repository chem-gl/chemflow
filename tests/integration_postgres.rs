use async_trait::async_trait;
use chemflow_rust::{
    data::family::MoleculeFamily,
    repository::WorkflowExecutionRepository,
    workflow::{
        manager::WorkflowManager,
        step::{StepExecutionInfo, StepInput, StepOutput, StepStatus, WorkflowStep},
    },
};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

// Dummy step that emits a result to test result_type persistence
struct AggregationLikeStep {
    id: Uuid,
}

#[async_trait]
impl WorkflowStep for AggregationLikeStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    fn get_name(&self) -> &str {
        "agg"
    }
    fn get_description(&self) -> &str {
        "agg test"
    }
    fn get_required_input_types(&self) -> Vec<String> {
        vec![]
    }
    fn get_output_types(&self) -> Vec<String> {
        vec!["aggregation_result".into()]
    }
    fn allows_branching(&self) -> bool {
        true
    }
    async fn execute(&self,
                     _input: StepInput,
                     _m: &HashMap<String, Box<dyn chemflow_rust::providers::molecule::traitmolecule::MoleculeProvider>>,
                     _p: &HashMap<String, Box<dyn chemflow_rust::providers::properties::trait_properties::PropertiesProvider>>,
                     _d: &HashMap<String, Box<dyn chemflow_rust::providers::data::trait_dataprovider::DataProvider>>)
                     -> Result<StepOutput, Box<dyn std::error::Error>> {
        let mut results = HashMap::new();
        results.insert("aggregation".to_string(), serde_json::json!({"count":0}));
        Ok(StepOutput { families: vec![],
                        results,
                        execution_info: StepExecutionInfo { step_id: self.id,
                                                            parameters: HashMap::new(),
                                                            parameter_hash: Some(chemflow_rust::database::repository::compute_sorted_hash(&HashMap::<String, serde_json::Value>::new())),
                                                            providers_used: vec![],
                                                            start_time: chrono::Utc::now(),
                                                            end_time: chrono::Utc::now(),
                                                            status: StepStatus::Completed,
                                                            root_execution_id: Uuid::new_v4(),
                                                            parent_step_id: None,
                                                            branch_from_step_id: None,
                                                            input_family_ids: vec![] } })
    }
}

#[tokio::test]
async fn test_postgres_migrations_and_result_type() -> Result<(), Box<dyn std::error::Error>> {
    // Skip if DATABASE_URL not set (CI condition)
    let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
    if db_url.is_empty() {
        eprintln!("DATABASE_URL not set; skipping integration test");
        return Ok(());
    }

    let pool = sqlx::PgPool::connect(&db_url).await?;
    // Run pending migrations (simple approach: execute each file in order 000*.sql)
    // Assumes tests run from workspace root
    for mig in ["0001_init.sql",
                "0002_relationships.sql",
                "0003_properties_and_results.sql",
                "0004_molecules.sql",
                "0006_parameter_hash_and_freeze.sql",
                "0007_step_results_type.sql",
                "0008_providers_used_gin_index.sql"]
    {
        let sql = std::fs::read_to_string(format!("migrations/{mig}"))?;
        for stmt in sql.split(";\n") {
            let s = stmt.trim();
            if !s.is_empty() {
                let _ = sqlx::query(s).execute(&pool).await;
            }
        }
    }

    let repo = WorkflowExecutionRepository::with_pool(pool).await;
    // Create a dummy family and persist
    let mut fam = MoleculeFamily::new("Fam".into(), None);
    fam.recompute_hash();
    repo.upsert_family(&fam).await?;
    // Execute aggregation-like persistence path
    let step = AggregationLikeStep { id: Uuid::new_v4() };
    let mut manager = WorkflowManager::new(repo.clone(), HashMap::new(), HashMap::new(), HashMap::new());
    let out = manager.execute_step(&step, vec![fam.clone()], HashMap::new()).await?;
    assert!(out.execution_info.parameter_hash.is_some());
    // Query result_type for inserted row
    if let Some(pool) = repo.clone().pool() {
        // access internal field via clone
        let row = sqlx::query("SELECT result_type FROM workflow_step_results WHERE step_id = $1 AND result_key='aggregation'").bind(out.execution_info.step_id)
                                                                                                                              .fetch_one(pool)
                                                                                                                              .await?;
        let rt: String = row.try_get("result_type")?;
        assert_eq!(rt, "aggregation");
    }
    Ok(())
}
