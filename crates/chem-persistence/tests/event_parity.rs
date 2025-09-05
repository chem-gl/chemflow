use chem_core::{build_flow_definition, model::ExecutionContext, step, EventStore, FlowEventKind, FlowRepository, InMemoryEventStore, InMemoryFlowRepository};
use chem_persistence::{pg::{build_pool, PgEventStore, PoolProvider, PgFlowRepository}, config::DbConfig};
use uuid::Uuid;

// Step dummy para probar.
struct DummyStep { id_s: &'static str }
impl step::StepDefinition for DummyStep {
    fn id(&self) -> &str { self.id_s }
    fn kind(&self) -> step::StepKind { step::StepKind::Source }
    fn run(&self, _ctx: &ExecutionContext) -> step::StepRunResult {
        step::StepRunResult::Success { outputs: vec![] }
    }
    fn base_params(&self) -> serde_json::Value { serde_json::Value::Null }
    fn name(&self) -> &str { self.id() }
}

#[test]
fn parity_inmemory_vs_pg() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("DATABASE_URL no definido: omitiendo parity test"); return; }
    // Encapsulamos todo en un scope para asegurar drop ordenado del pool antes del fin del proceso.
    {
        // InMemory run
        let mut mem_store = InMemoryEventStore::default();
        let mem_repo = InMemoryFlowRepository::new();
        let flow_id = Uuid::new_v4();
        let steps: Vec<Box<dyn step::StepDefinition>> = vec![Box::new(DummyStep{id_s: "s1"})];
        let def = build_flow_definition(&["s1"], steps);
        mem_store.append_kind(flow_id, FlowEventKind::FlowInitialized { definition_hash: def.definition_hash.clone(), step_count: def.len() });
        mem_store.append_kind(flow_id, FlowEventKind::StepStarted { step_index: 0, step_id: "s1".into() });
        mem_store.append_kind(flow_id, FlowEventKind::StepFinished { step_index: 0, step_id: "s1".into(), outputs: vec![], fingerprint: "fp1".into() });
        mem_store.append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint: "fp1".into() });
        let mem_events = mem_store.list(flow_id);

        // Postgres run
        let cfg = DbConfig::from_env();
        let pool = build_pool(&cfg.url, cfg.min_connections, cfg.max_connections).expect("pool");
        let mut pg_store = PgEventStore::new(PoolProvider{ pool });
        for e in &mem_events { pg_store.append_kind(flow_id, e.kind.clone()); }
        let pg_events = pg_store.list(flow_id);

        assert_eq!(mem_events.len(), pg_events.len(), "conteo eventos");
        for (a,b) in mem_events.iter().zip(pg_events.iter()) {
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
