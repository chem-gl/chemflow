use chem_core::event::FlowEventKind;
use chem_core::EventStore; // bring trait into scope for append_kind/list
use chem_persistence::pg::build_pool;
use chem_persistence::ConnectionProvider; // trait for provider.connection()
use chem_persistence::{PgEventStore, PoolProvider};
use diesel::prelude::*;
use diesel::sql_query;
use std::env;
use uuid::Uuid;

#[test]
fn branch_created_persists_and_is_listed() -> Result<(), Box<dyn std::error::Error>> {
    // Skip integration test when no DATABASE_URL is provided in environment.
    let database_url = match env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("Skipping DB integration test: DATABASE_URL not set");
            return Ok(());
        }
    };

    // Build pool and run migrations (build_pool executes migrations on first
    // checkout)
    let pool = build_pool(&database_url, 1, 2)?;
    let provider = PoolProvider { pool: pool.clone() };

    // Create PgEventStore backed by the pool provider
    let mut store = PgEventStore::new(provider);

    // Prepare ids and event
    let parent_flow = Uuid::new_v4();
    let branch_id = Uuid::new_v4();

    let kind = FlowEventKind::BranchCreated { branch_id,
                                              parent_flow_id: parent_flow,
                                              root_flow_id: parent_flow,
                                              created_from_step_id: "step_1".to_string(),
                                              divergence_params_hash: Some("deadbeef".to_string()) };

    // Append the BranchCreated event under the parent flow id
    let ev = EventStore::append_kind(&mut store, parent_flow, kind.clone());
    assert!(ev.seq > 0, "expected appended event to have seq>0");

    // The event should be visible via list()
    let events = EventStore::list(&store, parent_flow);
    let found = events.iter().any(|e| match &e.kind {
                                 FlowEventKind::BranchCreated { branch_id: bid, .. } => bid == &branch_id,
                                 _ => false,
                             });
    assert!(found, "BranchCreated event not found in event_store.list()");

    // Verify there is a row in workflow_branches for the inserted branch
    // We use a simple COUNT(*) query to avoid importing table mappings in the test.
    let mut conn = ConnectionProvider::connection(&store.provider)?;
    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let sql = format!("SELECT count(*) as count FROM workflow_branches WHERE branch_id = '{}'",
                      branch_id);
    let cr: CountRow = sql_query(sql).get_result(&mut *conn)?;
    assert_eq!(cr.count, 1, "expected one row in workflow_branches for branch_id");
    std::thread::sleep(std::time::Duration::from_millis(100));

    Ok(())
}
