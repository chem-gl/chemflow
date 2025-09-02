use chemflow_rust::workflow::step::MoleculeAcquisitionStep;
use chemflow_rust::providers::molecule::implementations::test_provider::TestMoleculeProvider;
use chemflow_rust::workflow::manager::WorkflowManager;
use chemflow_rust::database::repository::WorkflowExecutionRepository;
use std::collections::HashMap;
use serde_json::json;

#[tokio::test]
async fn test_persist_molecules_and_snapshot() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("DATABASE_URL not set; skipping integration test"); return; }
    let pool = chemflow_rust::config::create_pool().await.expect("pool");
    chemflow_rust::migrations::run_migrations().await.expect("migrations");
    let repo = WorkflowExecutionRepository::with_pool(pool).await;
    let mut mol_providers = HashMap::new(); mol_providers.insert("test_molecule".into(), Box::new(TestMoleculeProvider::new()) as _);
    let props = HashMap::new(); let data = HashMap::new();
    let mut manager = WorkflowManager::new(repo, mol_providers, props, data);
    let step = MoleculeAcquisitionStep { id: uuid::Uuid::new_v4(), name: "Acquire".into(), description: "Acquire".into(), provider_name: "test_molecule".into(), parameters: HashMap::from([("count".into(), json!(3))]) };
    let out = manager.execute_step(&step, vec![], step.parameters.clone()).await.expect("execute");
    assert_eq!(out.families.len(), 1);
    // Query DB counts
    let pool_ref = manager.repository().pool().unwrap();
    let (mol_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM molecules").fetch_one(pool_ref).await.unwrap();
    assert!(mol_count >= 3, "Expected at least 3 molecules persisted, got {}", mol_count);
    // Snapshot result presence
    let (snap_exists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workflow_step_results WHERE step_id = $1 AND result_key = 'snapshot_molecule_counts'")
        .bind(out.execution_info.step_id).fetch_one(pool_ref).await.unwrap();
    assert_eq!(snap_exists, 1, "Snapshot result missing");
}
