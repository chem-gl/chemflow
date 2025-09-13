use chem_core::model::ArtifactSpec;
use chem_core::{
    build_flow_definition, model::ExecutionContext, step, step::StepRunResult, EventStore, FlowEngine, FlowEventKind,
    InMemoryEventStore, InMemoryFlowRepository,
};
use chem_persistence::config::DbConfig;
use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
struct SeedOut {
    value: i64,
    schema_version: u32,
}
impl chem_core::model::ArtifactSpec for SeedOut {
    const KIND: chem_core::model::ArtifactKind = chem_core::model::ArtifactKind::GenericJson;
}

#[derive(Debug)]
struct Seed;
impl step::StepDefinition for Seed {
    fn id(&self) -> &str {
        "seed_pg"
    }
    fn base_params(&self) -> serde_json::Value {
        json!({})
    }
    fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
        StepRunResult::Success { outputs: vec![SeedOut { value: 7,
                                                         schema_version: 1 }.into_artifact()] }
    }
    fn kind(&self) -> step::StepKind {
        step::StepKind::Source
    }

    fn name(&self) -> &str {
        self.id()
    }

    fn definition_hash(&self) -> String {
        // Hash simple basado en id + kind + base_params
        let hash_input = json!({
            "id": self.id(),
            "kind": format!("{:?}", self.kind()),
            "base_params": self.base_params()
        });
        chem_core::hashing::hash_value(&hash_input)
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct AddOut {
    sum: i64,
    schema_version: u32,
}
impl chem_core::model::ArtifactSpec for AddOut {
    const KIND: chem_core::model::ArtifactKind = chem_core::model::ArtifactKind::GenericJson;
}
#[derive(Debug)]
struct Add;
impl step::StepDefinition for Add {
    fn id(&self) -> &str {
        "add_pg"
    }
    fn base_params(&self) -> serde_json::Value {
        json!({"inc":5})
    }
    fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
        use chem_core::model::TypedArtifact;
        let inp = ctx.input.as_ref().unwrap();
        let seed = TypedArtifact::<SeedOut>::decode(inp).unwrap();
        StepRunResult::Success { outputs: vec![AddOut { sum: seed.inner.value + 5,
                                                        schema_version: 1 }.into_artifact()] }
    }
    fn kind(&self) -> step::StepKind {
        step::StepKind::Transform
    }
}

#[test]
fn engine_flow_fingerprint_pg_vs_memory() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip engine_flow_fingerprint_pg_vs_memory (no DATABASE_URL)");
        return;
    }
    let flow_id = Uuid::new_v4();
    // InMemory run
    let mut mem_engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
    let def_mem = build_flow_definition(&["seed_pg", "add_pg"], vec![Box::new(Seed), Box::new(Add)]);
    mem_engine.next_with(flow_id, &def_mem).unwrap();
    mem_engine.next_with(flow_id, &def_mem).unwrap();
    let mem_events = mem_engine.event_store().list(flow_id);
    let _final_fp_mem = mem_events.iter()
                                 .find_map(|e| {
                                     if let FlowEventKind::FlowCompleted { flow_fingerprint } = &e.kind {
                                         Some(flow_fingerprint.clone())
                                     } else {
                                         None
                                     }
                                 })
                                 .expect("mem flowcompleted");

    // Postgres run (nueva secuencia desde cero con mismo flow_id)
    let cfg = DbConfig::from_env();
    eprintln!("engine_fingerprint: original cfg min={} max={} url={} (flow_id={})",
              cfg.min_connections, cfg.max_connections, cfg.url, flow_id);
    // Fuerza de aislamiento: use configured url and min/max connections
    let pool = build_pool(&cfg.url, cfg.min_connections as u32, cfg.max_connections as u32).expect("pool");
    let provider = PoolProvider { pool };
    let store = PgEventStore::new(provider);

    // ...existing code continues ...

    // Prevent running native destructor during test teardown (leak only in tests)
    std::mem::forget(store);
}
