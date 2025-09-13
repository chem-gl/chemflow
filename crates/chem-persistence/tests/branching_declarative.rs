use chem_core::event::FlowEventKind;
use chem_core::repo::build_flow_definition;
use chem_core::{EventStore, FlowRepository};
use chem_persistence::pg::build_pool;
use chem_persistence::ConnectionProvider;
use chem_persistence::{PgEventStore, PgFlowRepository, PoolProvider};
use diesel::prelude::*;
use diesel::sql_query;
use std::env;
use uuid::Uuid;

/// Small declarative helpers for tests
struct TestEnv {
    store: PgEventStore<PoolProvider>,
    repo: PgFlowRepository,
}

impl TestEnv {
    fn init() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let database_url = match env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        let pool = build_pool(&database_url, 1, 2)?;
        let provider = PoolProvider { pool: pool.clone() };
        let store = PgEventStore::new(provider);
        let repo = PgFlowRepository::new();
        Ok(Some(Self { store, repo }))
    }

    fn create_flow(&mut self, steps: &[&str]) -> Uuid {
        let flow = Uuid::new_v4();
        let def = build_flow_definition(steps, vec![]);
        let init = FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                    step_count: def.steps.len() };
        let _ = EventStore::append_kind(&mut self.store, flow, init);
        flow
    }

    fn create_branch(&mut self, parent: Uuid, from_step: &str, divergence: Option<String>) -> Uuid {
        let branch = Uuid::new_v4();
        let b = FlowEventKind::BranchCreated { branch_id: branch,
                                               parent_flow_id: parent,
                                               root_flow_id: parent,
                                               created_from_step_id: from_step.to_string(),
                                               divergence_params_hash: divergence };
        let _ = EventStore::append_kind(&mut self.store, parent, b);
        // Simulate engine copying initialization to branch
        let def = build_flow_definition(&[from_step], vec![]);
        let init = FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                    step_count: def.steps.len() };
        let _ = EventStore::append_kind(&mut self.store, branch, init);
        branch
    }

    fn row_count_for_branch(&self, branch: Uuid) -> Result<i64, Box<dyn std::error::Error>> {
        let mut conn = self.store.provider.connection()?;
        #[derive(diesel::QueryableByName)]
        struct CountRow {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }
        let sql = format!("SELECT count(*) as count FROM workflow_branches WHERE branch_id = '{}'",
                          branch);
        let cr: CountRow = sql_query(sql).get_result(&mut *conn)?;
        Ok(cr.count)
    }

    fn rehydrate(&self, branch: Uuid, def_steps: &[&str]) {
        let def = build_flow_definition(def_steps, vec![]);
        let events = self.store.list(branch);
        let instance = self.repo.load(branch, &events, &def);
        assert_eq!(instance.id, branch);
        assert_eq!(instance.steps.len(), def.steps.len());
    }
}

#[test]
fn branching_declarative_helpers_save_and_restore() -> Result<(), Box<dyn std::error::Error>> {
    let mut env = match TestEnv::init()? {
        Some(e) => e,
        None => {
            eprintln!("Skipping DB integration test: DATABASE_URL not set");
            return Ok(());
        }
    };

    // Create parent flow declaratively
    let parent = env.create_flow(&["s1", "s2", "s3"]);

    // Create branch declaratively from step s2
    let branch = env.create_branch(parent, "s2", Some("divhash123".to_string()));

    // Verify DB row exists
    let cnt = env.row_count_for_branch(branch)?;
    assert_eq!(cnt, 1, "expected one branch row");

    // Verify rehydration works and steps count preserved (we used single-step def
    // for branch init)
    env.rehydrate(branch, &["s2"]);

    Ok(())
}
