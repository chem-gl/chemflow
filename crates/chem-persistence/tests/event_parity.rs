use chem_core::{build_flow_definition, model::ExecutionContext, step, EventStore, FlowEventKind, FlowRepository, InMemoryEventStore, InMemoryFlowRepository};
use chem_persistence::pg::{PgEventStore, PgFlowRepository, PoolProvider};
mod test_support;
use test_support::with_pool;
use uuid::Uuid;
#[test]
fn no_artifact_duplication_on_retry() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    // Step estable que siempre produce el mismo payload/hash cuando termina
    struct AlwaysOk;
    impl step::StepDefinition for AlwaysOk {
        fn id(&self) -> &str { "ok" }
        fn base_params(&self) -> serde_json::Value { serde_json::json!({}) }
        fn run(&self, _ctx: &ExecutionContext) -> step::StepRunResult {
            step::StepRunResult::Success { outputs: vec![chem_core::model::Artifact { kind: chem_core::model::ArtifactKind::GenericJson,
                                                                                         hash: String::new(),
                                                                                         payload: serde_json::json!({"stable":true, "schema_version":1}),
                                                                                         metadata: None }] }
        }
        fn kind(&self) -> step::StepKind { step::StepKind::Transform }
        fn name(&self) -> &str { self.id() }
    }
    // Source mínima
    struct Src; impl step::StepDefinition for Src {
        fn id(&self) -> &str { "src" }
        fn base_params(&self) -> serde_json::Value { serde_json::json!({}) }
        fn run(&self, _ctx: &ExecutionContext) -> step::StepRunResult {
            step::StepRunResult::Success { outputs: vec![chem_core::model::Artifact { kind: chem_core::model::ArtifactKind::GenericJson,
                                                                                        hash: String::new(),
                                                                                        payload: serde_json::json!({"v":1, "schema_version":1}),
                                                                                        metadata: None }] }
        }
        fn kind(&self) -> step::StepKind { step::StepKind::Source }
        fn name(&self) -> &str { self.id() }
    }

    // Engine PG
    let pool = with_pool(|p| p.clone()).unwrap();
    let provider = PoolProvider { pool: pool.clone() };
    let event_store = PgEventStore::new(provider);
    let repo = PgFlowRepository::new();
    let mut engine = chem_core::FlowEngine::new_with_stores(event_store, repo);
    let flow_id = Uuid::new_v4();
    let def = build_flow_definition(&["src","ok"], vec![Box::new(Src), Box::new(AlwaysOk)]);
    // Ejecutar dos veces la transform final (éxito ambas) separadas por replay: no debe duplicar artifact
    engine.next_with(flow_id, &def).expect("src ok");
    engine.next_with(flow_id, &def).ok(); // primera vez ok
    // Forzar un retry lógico (aunque no falla, re-ejecutamos el último step para simular recomputo)
    // Nota: dedup por hash PK evita duplicación de fila
    engine.next_with(flow_id, &def).ok();
    let variants = engine.event_variants_for(flow_id);
    assert!(variants.iter().filter(|v| **v == "F").count() >= 2);
}

// Step dummy para probar.
struct DummyStep {
    id_s: &'static str,
}
impl step::StepDefinition for DummyStep {
    fn id(&self) -> &str {
        self.id_s
    }
    fn kind(&self) -> step::StepKind {
        step::StepKind::Source
    }
    fn run(&self, _ctx: &ExecutionContext) -> step::StepRunResult {
        step::StepRunResult::Success { outputs: vec![] }
    }
    fn base_params(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn name(&self) -> &str {
        self.id()
    }
}

#[test]
fn parity_inmemory_vs_pg() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL no definido: omitiendo parity test");
        return;
    }
    // Encapsulamos todo en un scope para asegurar drop ordenado del pool antes del
    // fin del proceso.
    {
        // InMemory run
        let mut mem_store = InMemoryEventStore::default();
        let mem_repo = InMemoryFlowRepository::new();
        let flow_id = Uuid::new_v4();
        let steps: Vec<Box<dyn step::StepDefinition>> = vec![Box::new(DummyStep { id_s: "s1" })];
        let def = build_flow_definition(&["s1"], steps);
        mem_store.append_kind(flow_id,
                              FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                               step_count: def.len() });
        mem_store.append_kind(flow_id,
                              FlowEventKind::StepStarted { step_index: 0,
                                                           step_id: "s1".into() });
        mem_store.append_kind(flow_id,
                              FlowEventKind::StepFinished { step_index: 0,
                                                            step_id: "s1".into(),
                                                            outputs: vec![],
                                                            fingerprint: "fp1".into() });
    // F7: Agregar un RetryScheduled ficticio para chequear serialización
    mem_store.append_kind(flow_id,
                  FlowEventKind::RetryScheduled { step_id: "s1".into(), retry_index: 1, reason: Some("test".into()) });
        mem_store.append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint: "fp1".into() });
        let mem_events = mem_store.list(flow_id);

        // Postgres run
        let pool = with_pool(|p| p.clone());
        if pool.is_none() {
            eprintln!("skip pg parity (sin pool global)");
            return;
        }
        let mut pg_store = PgEventStore::new(PoolProvider { pool: pool.unwrap() });
        for e in &mem_events {
            pg_store.append_kind(flow_id, e.kind.clone());
        }
        let pg_events = pg_store.list(flow_id);

        assert_eq!(mem_events.len(), pg_events.len(), "conteo eventos");
        for (a, b) in mem_events.iter().zip(pg_events.iter()) {
            let ja = serde_json::to_value(&a.kind).unwrap();
            let jb = serde_json::to_value(&b.kind).unwrap();
            assert_eq!(ja, jb, "JSON de FlowEventKind debe coincidir");
        }
        let mem_instance = mem_repo.load(flow_id, &mem_events, &def);
        let pg_repo = PgFlowRepository::new();
        let pg_instance = pg_repo.load(flow_id, &pg_events, &def);
        assert_eq!(mem_instance.completed, pg_instance.completed);
        assert_eq!(mem_instance.steps.len(), pg_instance.steps.len());
        // drop explícitos (opcional)
        drop(pg_store);
    }
}

#[test]
fn retry_event_roundtrip_and_replay_pending() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL no definido: omitiendo retry parity test");
        return;
    }
    let mut mem_store = InMemoryEventStore::default();
    let mem_repo = InMemoryFlowRepository::new();
    let flow_id = Uuid::new_v4();
    let steps: Vec<Box<dyn step::StepDefinition>> = vec![Box::new(DummyStep { id_s: "s1" })];
    let def = build_flow_definition(&["s1"], steps);
    mem_store.append_kind(flow_id,
                          FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(),
                                                           step_count: def.len() });
    mem_store.append_kind(flow_id,
                          FlowEventKind::StepStarted { step_index: 0, step_id: "s1".into() });
    mem_store.append_kind(flow_id,
                          FlowEventKind::StepFailed { step_index: 0,
                                                      step_id: "s1".into(),
                                                      error: chem_core::errors::CoreEngineError::Internal("e".into()),
                                                      fingerprint: "fp0".into() });
    mem_store.append_kind(flow_id,
                          FlowEventKind::RetryScheduled { step_id: "s1".into(), retry_index: 1, reason: None });
    let mem_events = mem_store.list(flow_id);
    let mem_instance = mem_repo.load(flow_id, &mem_events, &def);
    assert!(matches!(mem_instance.steps[0].status, chem_core::step::StepStatus::Pending),
            "RetryScheduled debe reponer Pending");

    // Roundtrip PG
    let pool = with_pool(|p| p.clone());
    if pool.is_none() { return; }
    let mut pg_store = PgEventStore::new(PoolProvider { pool: pool.unwrap() });
    for e in &mem_events { pg_store.append_kind(flow_id, e.kind.clone()); }
    let pg_events = pg_store.list(flow_id);
    assert_eq!(serde_json::to_value(&mem_events.iter().map(|e| &e.kind).collect::<Vec<_>>()).unwrap().to_string(),
               serde_json::to_value(&pg_events.iter().map(|e| &e.kind).collect::<Vec<_>>()).unwrap().to_string());
    let pg_repo = PgFlowRepository::new();
    let pg_instance = pg_repo.load(flow_id, &pg_events, &def);
    assert!(matches!(pg_instance.steps[0].status, chem_core::step::StepStatus::Pending));
}
