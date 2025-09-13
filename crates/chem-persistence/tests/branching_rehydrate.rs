use chem_core::event::FlowEventKind;
use chem_core::repo::build_flow_definition;
use chem_core::EventStore;
use chem_core::FlowRepository;
use chem_persistence::pg::build_pool;
use chem_persistence::{PgEventStore, PgFlowRepository, PoolProvider};
use std::env;
use uuid::Uuid;

#[test]
fn branch_rehydrates_via_pg_flow_repository() -> Result<(), Box<dyn std::error::Error>> {
    // Skip if no DATABASE_URL
    let database_url = match env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("Skipping DB integration test: DATABASE_URL not set");
            return Ok(());
        }
    };

    let pool = build_pool(&database_url, 1, 2)?;
    let provider = PoolProvider { pool: pool.clone() };
    let mut store = PgEventStore::new(provider);
    let repo = PgFlowRepository::new();

    // Setup: create a flow (parent) with FlowInitialized event
    let parent_flow = Uuid::new_v4();
    let def = build_flow_definition(&["s1", "s2"], vec![]);
    let init = FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                step_count: def.steps.len() };
    let _ev = EventStore::append_kind(&mut store, parent_flow, init);

    // Simulate engine copying events to branch: create branch id and copy
    // FlowInitialized
    let branch_id = Uuid::new_v4();
    let branch_event = FlowEventKind::BranchCreated { branch_id,
                                                      parent_flow_id: parent_flow,
                                                      root_flow_id: parent_flow,
                                                      created_from_step_id: "s1".to_string(),
                                                      divergence_params_hash: None };
    let _b_ev = EventStore::append_kind(&mut store, parent_flow, branch_event);

    // Also append the FlowInitialized event under the branch (as engine would copy)
    let init2 = FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                 step_count: def.steps.len() };
    let _ = EventStore::append_kind(&mut store, branch_id, init2);

    // Load events for branch and use PgFlowRepository to rehydrate
    let events = store.list(branch_id);
    let instance = repo.load(branch_id, &events, &def);

    // Basic checks: instance.flow_id == branch_id and step slots length ==
    // step_count
    // FlowInstance fields are `id` and `steps` (not `flow_id` / `slots`).
    assert_eq!(instance.id, branch_id);
    assert_eq!(instance.steps.len(), def.steps.len());

    Ok(())
}
