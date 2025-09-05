use chem_core::model::ArtifactSpec;
use chem_core::{FlowEngine, build_flow_definition, step, model::ExecutionContext, step::StepRunResult, EventStore, FlowEventKind, InMemoryEventStore, InMemoryFlowRepository};
use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider, PgFlowRepository};
use chem_persistence::config::DbConfig;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)] struct SeedOut { value: i64, schema_version: u32 }
impl chem_core::model::ArtifactSpec for SeedOut { const KIND: chem_core::model::ArtifactKind = chem_core::model::ArtifactKind::GenericJson; }

struct Seed; impl step::StepDefinition for Seed {
    fn id(&self) -> &str { "seed_pg" }
    fn base_params(&self) -> serde_json::Value { json!({}) }
    fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![SeedOut { value: 7, schema_version:1 }.into_artifact()] } }
    fn kind(&self) -> step::StepKind { step::StepKind::Source }
}
#[derive(Clone, Serialize, Deserialize)] struct AddOut { sum: i64, schema_version: u32 }
impl chem_core::model::ArtifactSpec for AddOut { const KIND: chem_core::model::ArtifactKind = chem_core::model::ArtifactKind::GenericJson; }
struct Add; impl step::StepDefinition for Add {
    fn id(&self) -> &str { "add_pg" }
    fn base_params(&self) -> serde_json::Value { json!({"inc":5}) }
    fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
        use chem_core::model::TypedArtifact; let inp = ctx.input.as_ref().unwrap(); let seed = TypedArtifact::<SeedOut>::decode(inp).unwrap();
        StepRunResult::Success { outputs: vec![AddOut { sum: seed.inner.value + 5, schema_version:1 }.into_artifact()] }
    }
    fn kind(&self) -> step::StepKind { step::StepKind::Transform }
}

#[test]
fn engine_flow_fingerprint_pg_vs_memory() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("skip engine_flow_fingerprint_pg_vs_memory (no DATABASE_URL)"); return; }
    let flow_id = Uuid::new_v4();
    // InMemory run
    let mut mem_engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
    let def_mem = build_flow_definition(&["seed_pg","add_pg"], vec![Box::new(Seed), Box::new(Add)]);
    mem_engine.next(flow_id, &def_mem).unwrap();
    mem_engine.next(flow_id, &def_mem).unwrap();
    let mem_events = mem_engine.event_store.list(flow_id);
    let final_fp_mem = mem_events.iter().find_map(|e| if let FlowEventKind::FlowCompleted { flow_fingerprint } = &e.kind { Some(flow_fingerprint.clone()) } else { None }).expect("mem flowcompleted");

    // Postgres run (nueva secuencia desde cero con mismo flow_id)
    let cfg = DbConfig::from_env();
    eprintln!("engine_fingerprint: original cfg min={} max={} url={} (flow_id={})", cfg.min_connections, cfg.max_connections, cfg.url, flow_id);
    // Fuerza de aislamiento: usar siempre (1,1) independientemente de config externa para detectar si el crash desaparece.
    let pool = build_pool(&cfg.url, 1, 1).expect("pool 1x1");
    let provider = PoolProvider { pool };
    let mut pg_engine = FlowEngine::new(PgEventStore::new(provider), PgFlowRepository::new());
    let def_pg = build_flow_definition(&["seed_pg","add_pg"], vec![Box::new(Seed), Box::new(Add)]);
    pg_engine.next(flow_id, &def_pg).unwrap();
    pg_engine.next(flow_id, &def_pg).unwrap();
    let pg_events = pg_engine.event_store.list(flow_id);
    let final_fp_pg = pg_events.iter().find_map(|e| if let FlowEventKind::FlowCompleted { flow_fingerprint } = &e.kind { Some(flow_fingerprint.clone()) } else { None }).expect("pg flowcompleted");

    assert_eq!(final_fp_mem, final_fp_pg, "Flow fingerprint debe coincidir entre InMemory y Postgres");

    // Drops explícitos para observar si el crash ocurre durante liberación de recursos.
    drop(pg_engine); // fuerza drop antes de fin de test
    // Nota: el pool se droppea automáticamente junto con provider (poseído por PgEventStore), pero acá ya lo liberamos.
}
