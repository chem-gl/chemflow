use chem_core::model::{Artifact, ArtifactKind, ExecutionContext};
use chem_core::repo::build_flow_definition;
use chem_core::step::{StepDefinition, StepKind, StepRunResult, StepSignal};
use chem_core::{FlowEngine, FlowEventKind};
use chem_persistence::config::DbConfig;
use chem_persistence::pg::{build_pool, PgEventStore, PgFlowRepository, PoolProvider};
use serde_json::json;
use uuid::Uuid;

#[test]
fn pg_store_translates_reserved_signal_to_policy_event() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    // Step fuente que emite la señal reservada
    struct PolicySource;
    impl StepDefinition for PolicySource {
        fn id(&self) -> &str {
            "policy_src"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            let artifact = Artifact { kind: ArtifactKind::GenericJson,
                                      hash: String::new(),
                                      payload: json!({"dummy":true, "schema_version":1}),
                                      metadata: None };
            let data = json!({
                "property_key": "inchikey:XYZ|prop:foo",
                "policy_id": "max_score",
                "params_hash": "abcd1234",
                "rationale": {"score": 0.99}
            });
            StepRunResult::SuccessWithSignals { outputs: vec![artifact],
                                                signals: vec![StepSignal { signal:
                                                                               "PROPERTY_PREFERENCE_ASSIGNED".into(),
                                                                           data }] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Source
        }
    }

    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, 1, 2).expect("pool");
    let provider = PoolProvider { pool };
    let event_store = PgEventStore::new(provider);
    let repo = PgFlowRepository::new();
    // Usamos la API genérica en lugar del builder tipado (este test define un
    // StepDefinition neutro).
    let mut engine = FlowEngine::new_with_stores(event_store, repo);

    let flow_id = Uuid::new_v4();
    let def = build_flow_definition(&["policy_src"], vec![Box::new(PolicySource)]);
    engine.next_with(flow_id, &def).expect("engine next ok");

    let variants = engine.event_variants_for(flow_id);
    assert_eq!(variants, vec!["I", "S", "P", "F", "C"], "Secuencia debe incluir P");
    let events = engine.events_for(flow_id);
    assert!(events.iter()
                  .any(|e| matches!(e.kind, FlowEventKind::PropertyPreferenceAssigned { .. })),
            "Debe existir evento tipado P");
}
