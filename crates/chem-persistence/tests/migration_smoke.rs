use chem_core::EventStore;
use uuid::Uuid;

#[test]
fn migration_allows_branchcreated_event_type() {
    // Skip when DATABASE_URL not set
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set - skipping migration smoke test");
        return;
    }

    use chem_core::event::FlowEventKind;
    use chem_persistence::pg::{build_dev_pool_from_env, PgEventStore, PoolProvider};

    let pool = build_dev_pool_from_env().expect("build pool");
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);

    let flow_id = Uuid::new_v4();

    // Construct a minimal BranchCreated event and append it.
    let kind = FlowEventKind::BranchCreated { branch_id: Uuid::new_v4(),
                                              parent_flow_id: flow_id,
                                              root_flow_id: flow_id,
                                              created_from_step_id: "source_step".to_string(),
                                              divergence_params_hash: None };

    // append_kind will panic if the DB rejects the event (e.g. due to CHECK
    // constraint).
    let ev = store.append_kind(flow_id, kind.clone());

    // Verify the returned event is the BranchCreated we inserted.
    match ev.kind {
        FlowEventKind::BranchCreated { .. } => { /* success */ }
        _ => panic!("Appended event was not BranchCreated"),
    }

    // Prevent Drop-based teardown that may crash native libs in tests
    drop(store);
    // provider and pool were moved into store; nothing else to forget.
    std::thread::sleep(std::time::Duration::from_millis(100));
}
