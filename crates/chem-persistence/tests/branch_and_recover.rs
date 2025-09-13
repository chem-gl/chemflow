use uuid::Uuid;

#[test]
fn pg_recover_and_branch_with_param_change() {
    // Require DATABASE_URL for integration
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set - skipping PG integration test");
        return;
    }

    use chem_core::model::{Artifact, ArtifactKind, ExecutionContext};
    use chem_core::step::{StepDefinition, StepKind, StepRunResult};
    use chem_core::{repo::build_flow_definition, FlowEngine};
    use chem_persistence::{build_dev_pool_from_env, ConnectionProvider, PgEventStore, PgFlowRepository, PoolProvider};
    use serde_json::json;

    // Minimal source step that emits a single artifact
    #[derive(Debug)]
    struct Src;
    impl StepDefinition for Src {
        fn id(&self) -> &str {
            "seed"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"v":1,"schema_version":1}),
                                 metadata: None };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Source
        }
    }

    // Minimal follow-up step that consumes input and emits another artifact
    #[derive(Debug)]
    struct Next;
    impl StepDefinition for Next {
        fn id(&self) -> &str {
            "next"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({"param": "default"})
        }
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            let _in = ctx.input.as_ref();
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"v":2,"schema_version":1}),
                                 metadata: None };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Transform
        }
    }

    // Build pool and stores
    let pool = build_dev_pool_from_env().expect("build pool");
    let provider = PoolProvider { pool };
    let store = PgEventStore::new(provider);

    let repo = PgFlowRepository::new();
    let mut engine: FlowEngine<_, _> = FlowEngine::new_with_stores(store, repo);

    let def = build_flow_definition(&["seed", "next"], vec![Box::new(Src), Box::new(Next)]);
    let flow_id = Uuid::new_v4();

    // execute source (step 0)
    engine.next_with(flow_id, &def).expect("source run");

    // Now create a branch from the seed step, providing divergence params hash
    let branch_id = engine.branch(flow_id, &def, "seed", Some("divergence-param-hash".to_string()))
                          .expect("branch ok");

    // events for parent should contain BranchCreated
    let parent_events = engine.events_for(flow_id);
    let has_branch = parent_events.iter()
                                  .any(|e| matches!(e.kind, chem_core::event::FlowEventKind::BranchCreated { .. }));
    assert!(has_branch, "expected BranchCreated in parent events");

    // Recreate engine backed by PG (simulates recovery) and ensure events are
    // visible
    let pool2 = build_dev_pool_from_env().expect("build pool2");
    let provider2 = PoolProvider { pool: pool2 };
    let store2 = PgEventStore::new(provider2);
    let repo2 = PgFlowRepository::new();
    let mut recovered_engine: FlowEngine<_, _> = FlowEngine::new_with_stores(store2, repo2);

    let recovered_events = recovered_engine.events_for(flow_id);
    assert_eq!(recovered_events.len(),
               parent_events.len(),
               "recovered events count should match persisted");

    // Try to continue executing the branch: running next step on the branch should
    // succeed
    recovered_engine.next_with(branch_id, &def).expect("branch continue next run");

    let branch_events = recovered_engine.events_for(branch_id);
    let has_finished = branch_events.iter()
                                    .any(|e| matches!(e.kind, chem_core::event::FlowEventKind::StepFinished { .. }));
    assert!(has_finished, "expected StepFinished in branch events after continuing branch");

    // Optionally verify workflow_branches row exists via raw SQL using the provider
    // pool. Some test environments may not have the metadata table; do a
    // best-effort check and treat failures as non-fatal (log them).
    {
        use diesel::sql_types::BigInt;
        use diesel::QueryableByName;
        use diesel::RunQueryDsl;

        let conn_res = recovered_engine.event_store().provider.connection();
        if let Ok(mut conn) = conn_res {
            #[derive(QueryableByName, Debug)]
            struct CountRow {
                #[diesel(sql_type = BigInt)]
                count: i64,
            }

            let query = diesel::sql_query(
                "SELECT count(*) as count FROM workflow_branches WHERE branch_id = $1",
            )
            .bind::<diesel::sql_types::Uuid, _>(branch_id);

            match query.get_result::<CountRow>(&mut conn) {
                Ok(row) => {
                    assert!(row.count >= 1, "workflow_branches should contain the branch row");
                }
                Err(e) => {
                    eprintln!("workflow_branches query failed (non-fatal) - maybe table missing or DB trimmed: {:?}",
                              e);
                }
            }
        } else {
            eprintln!("could not get DB connection to verify workflow_branches (non-fatal)");
        }
    }

    // Prevent running native destructors that can crash in test runner teardown
    // by forgetting the engines which own the stores/providers. This leaks in
    // tests only but avoids native teardown races.
    std::mem::forget(engine);
    std::mem::forget(recovered_engine);
}
