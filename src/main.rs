/// Validación F7: Retry manual mínimo, schedule_retry, eventos y replay.
fn run_f7_validation() {
    use chem_core::{FlowEngine, build_flow_definition};
    use chem_core::repo::FlowRepository;
    use chem_core::step::{StepDefinition, StepKind, StepRunResult};
    use chem_core::event::FlowEventKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Step Source dummy para inicializar el flujo
    struct DummySource;
    impl StepDefinition for DummySource {
        fn id(&self) -> &str { "src" }
        fn kind(&self) -> StepKind { StepKind::Source }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> StepRunResult {
            // Return a minimal artifact so downstream transform steps have an input.
            let art = chem_core::model::Artifact {
                kind: chem_core::model::ArtifactKind::GenericJson,
                hash: String::new(),
                payload: serde_json::json!({ "schema_version": 1 }),
                metadata: None,
            };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn base_params(&self) -> serde_json::Value { serde_json::json!({}) }
        fn name(&self) -> &str { "src" }
    }

    // Step F7 que falla la primera vez y luego pasa (comparte un flag)
    struct F7Step {
        id: &'static str,
        failed_once: Rc<RefCell<bool>>,
    }
    impl F7Step {
        fn new_with_flag(id: &'static str, flag: Rc<RefCell<bool>>) -> Self {
            Self { id, failed_once: flag }
        }
        fn new(id: &'static str) -> Self {
            Self { id, failed_once: Rc::new(RefCell::new(false)) }
        }
    }
    impl StepDefinition for F7Step {
        fn id(&self) -> &str { self.id }
        fn kind(&self) -> StepKind { StepKind::Transform }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> StepRunResult {
            let mut failed = self.failed_once.borrow_mut();
            if !*failed {
                *failed = true;
                return StepRunResult::Failure { error: chem_core::errors::CoreEngineError::Internal("Fallo intencional F7".into()) };
            }
            StepRunResult::Success { outputs: vec![] }
        }
        fn base_params(&self) -> serde_json::Value { serde_json::json!({}) }
        fn name(&self) -> &str { self.id }
    }

    // Flag compartido entre instancias de F7Step
    let shared_flag = Rc::new(RefCell::new(false));
    let def = build_flow_definition(
        &["src", "f7step"],
        vec![Box::new(DummySource), Box::new(F7Step::new_with_flag("f7step", shared_flag.clone()))]
    );
    let mut engine = FlowEngine::new_with_stores(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    );
    let flow_id = engine.ensure_default_flow_id();

    // Ejecutar: primero se ejecuta el Source, luego el step F7 que debe fallar
    let r_src = engine.next_with(flow_id, &def);
    assert!(r_src.is_ok(), "F7: el Source debe ejecutarse OK primero");
    // Ahora ejecutar el step que falla la primera vez
    let res1 = engine.next_with(flow_id, &def);
    let events1 = engine.events().unwrap();
    let has_failed = res1.is_err() || events1.iter().any(|e| matches!(e.kind, FlowEventKind::StepFailed { .. }));
    assert!(has_failed, "F7: Debe haber StepFailed en eventos o next_with debe retornar Err");

    // Schedule retry manual (simula CLI):
    let retry_reason = Some("retry test".to_string());
    let def_for_retry = build_flow_definition(
        &["src", "f7step"],
        vec![Box::new(DummySource), Box::new(F7Step::new_with_flag("f7step", shared_flag.clone()))]
    );
    let retry_res = engine.schedule_retry(flow_id, &def_for_retry, "f7step", retry_reason.clone(), Some(1));
    let def_for_load = build_flow_definition(
        &["src", "f7step"],
        vec![Box::new(DummySource), Box::new(F7Step::new_with_flag("f7step", shared_flag.clone()))]
    );
    assert!(retry_res.is_ok(), "F7: schedule_retry debe funcionar");
    let events2 = engine.events().unwrap();
    let has_retry = events2.iter().any(|e| matches!(e.kind, FlowEventKind::RetryScheduled { .. }));
    assert!(has_retry, "F7: Debe haber RetryScheduled en eventos");

    // Ejecutar de nuevo: debe pasar ahora
    let res2 = engine.next_with(flow_id, &def);
    assert!(res2.is_ok(), "F7: El step debe pasar tras el retry");
    let events3 = engine.events().unwrap();
    let finished = events3.iter().any(|e| matches!(e.kind, FlowEventKind::StepFinished { .. }));
    assert!(finished, "F7: Debe haber StepFinished tras retry");

    // Replay: reconstruir el estado y verificar que el step está FinishedOk
    let events = engine.events().unwrap();
    let instance = chem_core::repo::InMemoryFlowRepository::new().load(flow_id, &events, &def_for_load);
    let slot = &instance.steps[1];
    assert_eq!(slot.status, chem_core::step::StepStatus::FinishedOk, "F7: Step debe estar FinishedOk tras retry");
    assert_eq!(slot.retry_count, 1, "F7: retry_count debe ser 1 tras un retry");

    println!("!Validación F7: OK (retry manual, eventos y replay)");
}

use chem_domain::Molecule;
use chem_core::step::StepKind;
use chem_core::FlowEngine;
use chem_core::{typed_artifact, typed_step};

use chem_persistence::{PgEventStore, PgFlowRepository, PoolProvider};
use chem_adapters::artifacts::FamilyPropertiesArtifact;
use chem_adapters::steps::acquire::AcquireMoleculesStep;
use chem_adapters::steps::compute::ComputePropertiesStep;
use chem_adapters::encoder::{DomainArtifactEncoder, SimpleDomainEncoder};
use chem_domain::MoleculeFamily;
use serde_json::to_string_pretty;
use uuid::Uuid;
use chem_adapters::steps::policy_demo::PolicyDemoStep;

// --------------------
// Artifactos tipados
// --------------------
typed_artifact!(TextOut { text: String });
typed_artifact!(CharsPas { chars: Vec<char> });
typed_artifact!(CountOut { count: usize });

// --------------------
// Steps tipados
// --------------------

// Steps tipados (macros)
typed_step! {
    source SeedStep {
        id: "seed_text",
        output: TextOut,
        params: (),
        fields { seed: String },
        run(me, _p) {
             let upper = me.seed.to_uppercase();
            TextOut { text: upper, schema_version: 1 }
        }
    }

}

typed_step! {
    step SplitStep {
        id: "split_chars",
        kind: StepKind::Transform,
        input: TextOut,
        output: CharsPas,
        params: (),
        run(_self, inp, _p) {
            let chars: Vec<char> = inp.text.chars().collect();
            CharsPas { chars, schema_version: 1 }
        }
    }
}

typed_step! {
    step ForwardStep {
        id: "forward_chars",
        kind: StepKind::Transform,
        input: CharsPas,
        output: CharsPas,
        params: (),
        run(_self, inp, _p) {
            CharsPas { chars: inp.chars, schema_version: 1 }
        }
    }
}

typed_step! {
    step PrintAndCountStep {
        id: "print_count",
        kind: StepKind::Sink,
        input: CharsPas,
        output: CountOut,
        params: (),
        run(_self, inp, _p) {
            let joined: String = inp.chars.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("-");
            println!("Chars: {}", joined);
            CountOut { count: joined.chars().filter(|c| *c != '-').count(), schema_version: 1 }
        }
    }
}

fn main() {
    // Cargar variables de entorno desde .env si existe (antes de leer DATABASE_URL)
    let _ = dotenvy::dotenv();
    //uso ejemplo de Tarea 1
    // Ejemplo de creación de moléculas usando SMILES
    let smiles_benzene = "C1=CC=CC=C1"; // Benceno
    let smiles_ethanol = "CCO"; // Etanol

    // Crear moléculas y manejar posibles errores
    let molecule1 = Molecule::new_molecule_with_smiles(smiles_benzene).expect("Error al crear la molécula 1");
    let molecule2 = Molecule::new_molecule_with_smiles(smiles_ethanol).expect("Error al crear la molécula 2");

    // Imprimir detalles de las moléculas
    println!("Molecula 1: {}", molecule1);
    println!("InChI de Molecula 1: {}", molecule1.inchi());

    println!("Molecula 2: {}", molecule2);
    println!("InChI de Molecula 2: {}", molecule2.inchi());

    // F4: uso del encoder dominio → artifact neutral (molecule y family)
    let encoder = SimpleDomainEncoder::default();
    let mol_art = encoder.encode_molecule(&molecule1);
    println!("[F4] Artifact molécula (kind={:?}) payload={}",
             mol_art.kind,
             to_string_pretty(&mol_art.payload).unwrap_or_default());
    // Construcción de familia determinista mínima con provenance estable
    let provenance = serde_json::json!({ "source": "main_demo", "version": 1 });
    let family = MoleculeFamily::new(vec![molecule1, molecule2], provenance).expect("family ok");
    println!("[F4] family_hash(dom): {}", family.family_hash());
    let fam_art = encoder.encode_family(&family);
    println!("[F4] Artifact familia (kind={:?}) payload={}",
             fam_art.kind,
             to_string_pretty(&fam_art.payload).unwrap_or_default());
    // Construir y ejecutar el flujo
    let mut engine = FlowEngine::new().firstStep(SeedStep::new("HolaMundo".to_string()))
                                      .add_step(SplitStep::new())
                                      .add_step(ForwardStep::new())
                                      .add_step(PrintAndCountStep::new())
                                      .build();
    engine.set_name("demo_chars");
    // Ejecutar hasta completar el flujo
    engine.run_to_end().expect("run ok");
    let variants = engine.event_variants().unwrap_or_default();
    println!("Secuencia de eventos F2: {:?}", variants);
    let events = engine.events().unwrap();
    let finished_count = events.iter()
                               .filter(|e| matches!(e.kind, chem_core::FlowEventKind::StepFinished { .. }))
                               .count();
    let completed = events.iter()
                          .any(|e| matches!(e.kind, chem_core::FlowEventKind::FlowCompleted { .. }));
    assert_eq!(finished_count, 4, "Deben terminar 4 steps");
    assert!(completed, "Debe existir FlowCompleted al final del flujo");    let flow_fp = engine.flow_fingerprint().unwrap_or_default();
    println!("Flow fingerprint agregado: {}", flow_fp);
    // Recupera el último output tipado del step final y lo imprime
    if let Some(Ok(out)) = engine.last_step_output_typed::<CountOut>("print_count") {
        println!("Cantidad de letras: {}", out.inner.count);
    }
    println!("!Validación F2: OK (flujo ejecutado y completado determinísticamente)");
    // validacion del flujo 3 (PG demo) – opt-in to avoid libpq/GSS crashes on some setups
    if std::env::var("CHEMFLOW_RUN_PG_DEMO").ok().as_deref() == Some("1") {
        maybe_run_pg_demo();
    } else {
        eprintln!("[PG DEMO] Skipping (set CHEMFLOW_RUN_PG_DEMO=1 to enable)");
    }
    // validacion del flujo 4
    println!("--- Iniciando validación F4 ---");
    {
        // Pipeline F4: Acquire (Source) → Compute (Transform)
        let mut engine4 = FlowEngine::new()
            .firstStep(AcquireMoleculesStep::new())
            .add_step(ComputePropertiesStep::new())
            .build();
        engine4.set_name("demo_f4_acquire_compute");
        engine4.run_to_end().expect("run ok");
        if let Some(Ok(out)) = engine4.last_step_output_typed::<FamilyPropertiesArtifact>("compute_properties") {
            println!("[F4] propiedades calculadas: {}", out.inner.items.len());
        }
        if let Some(fp) = engine4.flow_fingerprint() {
            println!("[F4] fingerprint: {}", fp);
        }
        let variants = engine4.event_variants().unwrap_or_default();
        println!("[F4] eventos: {:?}", variants);

        // Segunda corrida idéntica para demostrar determinismo de F4
        let mut engine4b = FlowEngine::new()
            .firstStep(AcquireMoleculesStep::new())
            .add_step(ComputePropertiesStep::new())
            .build();
        engine4b.set_name("demo_f4_acquire_compute");
        engine4b.run_to_end().expect("run ok");
        let fp_a = engine4.flow_fingerprint().unwrap_or_default();
        let fp_b = engine4b.flow_fingerprint().unwrap_or_default();
        println!("[F4] determinismo: fp_a == fp_b ? {}", fp_a == fp_b);
    }
    println!("--- Iniciando validación F5 ---");
    run_f5_lowlevel();
     println!("--- Iniciando validación F6 ---");
    if let Err(e) = run_f6_validation() {
        eprintln!("[F6] Error: {e}");
    } else {
        println!("[F6] Validación OK");
    }
    println!("--- Iniciando validación F7 ---");
    run_f7_validation();
    println!("--- iniciando validación F8 ---");
    if let Err(e) = run_f8_validation() {
        eprintln!("[F8] Error: {e}");
    } else {
        println!("[F8] Validación OK");
    }
    
}
/// Demo/validation for F8: append a StepFailed and verify errors persisted in `step_execution_errors`.
fn run_f8_validation() -> Result<(), String> {
    // Require DATABASE_URL (we run migrations via pool builder)
    if std::env::var("DATABASE_URL").is_err() {
        return Err("DATABASE_URL not set; cannot run F8 demo".into());
    }

    // Build pool and stores
    let pool = chem_persistence::build_dev_pool_from_env().map_err(|e| e.to_string())?;
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);

    use chem_core::{EventStore, FlowEventKind};

    let flow_id = Uuid::new_v4();

    // Create a StepFailed event; PgEventStore will persist the error row in the same transaction.
    let err = chem_core::errors::CoreEngineError::Internal("demo f8 internal".to_string());
    let kind = FlowEventKind::StepFailed {
        step_index: 0,
        step_id: "f8_step".to_string(),
        error: err.clone(),
        fingerprint: "fp_f8_demo".to_string(),
    };
    let _ev = store.append_kind(flow_id, kind);

    // Query persisted errors and print them
    let errors = store.list_errors(flow_id);
    if errors.is_empty() {
        return Err("no errors persisted for flow".into());
    }
    println!("[F8] persisted errors: {}", errors.len());
    for e in errors.iter() {
        println!("[F8] id={} flow_id={} step_id={} attempt={} class={} ts={} details={:?}",
                 e.id, e.flow_id, e.step_id, e.attempt_number, e.error_class, e.ts, e.details);
    }

    Ok(())
}
// Fuente mínima que emite un artifact compatible con DummyIn (policy_demo)
struct F6Seed;
impl chem_core::step::StepDefinition for F6Seed {
    fn id(&self) -> &str { "f6_seed" }
    fn base_params(&self) -> serde_json::Value { serde_json::json!({}) }
    fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> chem_core::step::StepRunResult {
        let art = chem_core::model::Artifact { kind: chem_core::model::ArtifactKind::GenericJson,
                                               hash: String::new(),
                                               payload: serde_json::json!({"v":1, "schema_version":1}),
                                               metadata: None };
        chem_core::step::StepRunResult::Success { outputs: vec![art] }
    }
    fn kind(&self) -> chem_core::step::StepKind { chem_core::step::StepKind::Source }
}

fn run_f6_validation() -> Result<(), String> {
    // Construir flujo: F6Seed (Source) -> PolicyDemoStep (Transform)
    let mut engine = FlowEngine::new_with_stores(chem_core::InMemoryEventStore::default(),
                                                 chem_core::InMemoryFlowRepository::new());
    let steps: Vec<Box<dyn chem_core::StepDefinition>> = vec![Box::new(F6Seed), Box::new(PolicyDemoStep::new())];
    let def = chem_core::repo::build_flow_definition(&["f6_seed", "policy_demo"], steps);
    let flow_id = Uuid::new_v4();
    engine.next_with(flow_id, &def).map_err(|e| e.to_string())?; // f6_seed
    engine.next_with(flow_id, &def).map_err(|e| e.to_string())?; // policy_demo
    // Verificar orden alrededor del step "policy_demo": Started -> P -> Finished
    let events = engine.events_for(flow_id);
    let idx_started = events.iter().enumerate().find_map(|(i, e)| match &e.kind {
        chem_core::FlowEventKind::StepStarted { step_id, .. } if step_id == "policy_demo" => Some(i),
        _ => None,
    }).ok_or_else(|| "no StepStarted(policy_demo)".to_string())?;
    let idx_finished = events.iter().enumerate().rev().find_map(|(i, e)| match &e.kind {
        chem_core::FlowEventKind::StepFinished { step_id, .. } if step_id == "policy_demo" => Some(i),
        _ => None,
    }).ok_or_else(|| "no StepFinished(policy_demo)".to_string())?;
    let idx_p = events.iter().enumerate().find_map(|(i, e)| match &e.kind {
        chem_core::FlowEventKind::PropertyPreferenceAssigned { .. } if i > idx_started => Some(i),
        _ => None,
    }).ok_or_else(|| "no se emitió evento P".to_string())?;
    if !(idx_started < idx_p && idx_p < idx_finished) {
        return Err("evento P debe ocurrir entre StepStarted y StepFinished de policy_demo".into());
    }
    // No debe existir StepSignal genérica con la señal reservada
    let had_reserved_signal = events.iter().any(|e| matches!(e.kind, chem_core::FlowEventKind::StepSignal { ref signal, .. } if signal=="PROPERTY_PREFERENCE_ASSIGNED"));
    if had_reserved_signal { return Err("Se encontró StepSignal genérica para señal reservada".into()); }
    Ok(())
}
mod pg_persistence_demo {
    use super::*;
    pub fn run() -> Result<(), String> {
        // Builder ergonómico con repositorio (Postgres) como en in-memory,
        // pasando los stores por el constructor del engine.
        let pool = chem_persistence::build_dev_pool_from_env().map_err(|e| e.to_string())?;
        let provider = PoolProvider { pool };
        let event_store = PgEventStore::new(provider);
        let repository = PgFlowRepository::new();
        let mut engine = FlowEngine::builder(event_store, repository).firstStep(SeedStep::new("HolaPG".to_string()))
                                                                     .add_step(SplitStep::new())
                                                                     .add_step(ForwardStep::new())
                                                                     .add_step(PrintAndCountStep::new())
                                                                     .build();
        engine.set_default_flow_name("demo_pg_chars");
        let _flow_id = engine.run_to_end_default_flow().map_err(|e| e.to_string())?;
        // 5) Inspección: listar variantes de eventos y fingerprint final (leídos desde
        //    Postgres) sin pasar flow_id explícito (usa default_flow_id del engine)
        let variants = engine.event_variants_default().unwrap_or_default();
        println!("[PG] Secuencia de eventos: {:?}", variants);
        if let Some(fp) = engine.flow_fingerprint_default() {
            println!("[PG] Flow fingerprint agregado: {}", fp);
        }
        let events = engine.events_default().unwrap_or_default();
        let finished = events.iter()
                             .filter(|e| matches!(e.kind, chem_core::FlowEventKind::StepFinished { .. }))
                             .count();
        let completed = events.iter()
                              .any(|e| matches!(e.kind, chem_core::FlowEventKind::FlowCompleted { .. }));
        println!("[PG] Verificación: eventos={}, finished={}, completed={}",
                 events.len(),
                 finished,
                 completed);
        if finished < 4 || !completed {
            return Err(format!("persistencia incompleta: finished={}, completed={}", finished, completed));
        }

        // 6) Recuperar el último output tipado del sink
        if let Some(Ok(out)) = engine.last_step_output::<CountOut>("print_count") {
            println!("[PG] Cantidad de letras: {}", out.inner.count);
        }

        Ok(())
    }
    pub fn run_replay_parity() -> Result<(), String> {
        // Pool único, dos motores separados que consultan el mismo backend persistente
        let pool = chem_persistence::build_dev_pool_from_env().map_err(|e| e.to_string())?;
        let provider1 = PoolProvider { pool: pool.clone() };
        let provider2 = PoolProvider { pool };

        let event_store1 = PgEventStore::new(provider1);
        let repo1 = PgFlowRepository::new();
        let event_store2 = PgEventStore::new(provider2);
        let repo2 = PgFlowRepository::new();

        let steps: Vec<Box<dyn chem_core::StepDefinition>> = vec![Box::new(SeedStep::new("HolaPG".to_string())),
                                                                  Box::new(SplitStep::new()),
                                                                  Box::new(ForwardStep::new()),
                                                                  Box::new(PrintAndCountStep::new()),];
        let definition_a = chem_core::repo::build_flow_definition_auto(steps);

        let mut engine1 = FlowEngine::new_with_definition(event_store1, repo1, definition_a);
        engine1.set_default_flow_name("demo_pg_replay");
        // Ejecutar el flujo una vez para materializar eventos en Postgres y obtener el flow_id
        let flow_id = engine1.run_to_end_default_flow().map_err(|e| e.to_string())?;
        let fp1 = engine1
            .flow_fingerprint_default()
            .ok_or_else(|| "no fingerprint from engine1".to_string())?;
        let variants1 = engine1.event_variants_default();
        println!("[PG] Replay demo - eventos engine1: {:?}", variants1);

        // Motor 2 (limpio) lee y compara fingerprint desde la misma DB
        let steps_b: Vec<Box<dyn chem_core::StepDefinition>> = vec![Box::new(SeedStep::new("HolaPG".to_string())),
                                                                    Box::new(SplitStep::new()),
                                                                    Box::new(ForwardStep::new()),
                                                                    Box::new(PrintAndCountStep::new()),];
        let definition_b = chem_core::repo::build_flow_definition_auto(steps_b);
        let mut engine2 = FlowEngine::new_with_definition(event_store2, repo2, definition_b);
        // Aseguramos que el segundo motor apunte al mismo flow_id para leer los eventos existentes
        engine2.set_default_flow_id(flow_id);
        let variants2 = engine2.event_variants_default();
        let fp2 = engine2
            .flow_fingerprint_default()
            .ok_or_else(|| "no fingerprint from engine2".to_string())?;
        println!("[PG] Replay demo - eventos engine2: {:?}", variants2);
        if fp1 == fp2 {
            println!("[PG] Replay parity OK: fingerprint coincide");
        } else {
            return Err("Replay parity mismatch".into());
        }
        Ok(())
    }
}

fn maybe_run_pg_demo() {
    // Ejecutar sólo si hay DATABASE_URL y aplicar mitigación para GSS por defecto.
    if let Ok(url) = std::env::var("DATABASE_URL") {
        // Si no hay gssencmode en la URL y el env no está seteado, deshabilitar GSS para evitar aborts en entornos con libpq+GSS.
        if !url.to_lowercase().contains("gssencmode=") && std::env::var("PGGSSENCMODE").is_err() {
            std::env::set_var("PGGSSENCMODE", "disable");
            eprintln!("[PG DEMO] PGGSSENCMODE=disable (auto) to evitar issues GSS/libpq");
        }
    } else {
        eprintln!("[PG DEMO] DATABASE_URL no definido; omitiendo demos PG");
        return;
    }
    if let Err(e) = pg_persistence_demo::run() {
        eprintln!("[PG DEMO] Error (basic): {e:?}");
    }
    if let Err(e) = pg_persistence_demo::run_replay_parity() {
        eprintln!("[PG DEMO] Error (replay): {e:?}");
    }
}

// Ejemplo F5: uso directo de PgEventStore/PgFlowRepository (append/list/replay)
fn run_f5_lowlevel() {
    // Bring the trait for repo.load into scope
    use chem_core::repo::FlowRepository;

    // Asegura cargar .env si aún no se cargó en este contexto
    let _ = dotenvy::dotenv();

    // Ejecutar sólo si hay DATABASE_URL definido
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("[F5] DATABASE_URL no definido; omitiendo ejemplo F5");
        return;
    }
    // Hint operabilidad: si usas libpq con GSSAPI, considera desactivar GSS encryption
    // añadiendo `?gssencmode=disable` a DATABASE_URL o exportando PGGSSENCMODE=disable
    // si observas errores de k5_mutex en teardown.
    if let Ok(url) = std::env::var("DATABASE_URL") {
        let gssen_env = std::env::var("PGGSSENCMODE").unwrap_or_default();
        if !url.to_lowercase().contains("gssencmode=") && gssen_env.is_empty() {
            std::env::set_var("PGGSSENCMODE", "disable");
            eprintln!("[F5] PGGSSENCMODE=disable (auto) para evitar issues GSS/libpq; añade gssencmode=disable a DATABASE_URL si prefieres");
        }
    }
    // 1) Pool + provider
    let pool = match chem_persistence::build_dev_pool_from_env() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[F5] Error construyendo pool: {e}");
            return;
        }
    };
    let provider = PoolProvider { pool };

    // 2) Instanciar store y repo Postgres
    let mut store = PgEventStore::new(provider);
    let repo = PgFlowRepository::new();

    // 3) Simular ejecución mínima (append-only) con artifact hash (64 hex)
    use chem_core::{EventStore, FlowEventKind};
    use uuid::Uuid;
    let flow_id = Uuid::new_v4();
    let output_hash = "f00df00df00df00df00df00df00df00df00df00df00df00df00df00df00df00d".to_string();
    store.append_kind(flow_id, FlowEventKind::FlowInitialized { definition_hash: "f5_demo_def".into(), step_count: 1 });
    store.append_kind(flow_id, FlowEventKind::StepStarted { step_index: 0, step_id: "f5_step".into() });
    store.append_kind(flow_id, FlowEventKind::StepFinished { step_index: 0,
                                                             step_id: "f5_step".into(),
                                                             outputs: vec![output_hash],
                                                             fingerprint: "fp_demo".into() });
    store.append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint: "fp_demo".into() });

    // 4) Listar eventos y mostrar variantes compactas
    let events = store.list(flow_id);
    let variants: Vec<&'static str> = events
        .iter()
        .map(|e| match e.kind {
            chem_core::FlowEventKind::FlowInitialized { .. } => "I",
            chem_core::FlowEventKind::StepStarted { .. } => "S",
            chem_core::FlowEventKind::StepFinished { .. } => "F",
            chem_core::FlowEventKind::StepFailed { .. } => "X",
            chem_core::FlowEventKind::StepSignal { .. } => "G",
            chem_core::FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
            chem_core::FlowEventKind::RetryScheduled { .. } => "R",
            chem_core::FlowEventKind::BranchCreated { .. } => "B",
            chem_core::FlowEventKind::FlowCompleted { .. } => "C",
        })
        .collect();
    println!("[F5] variantes lowlevel: {:?}", variants);

    // 5) Replay con PgFlowRepository usando una definición mínima
    struct DemoStep;
    impl chem_core::step::StepDefinition for DemoStep {
        fn id(&self) -> &str { "f5_step" }
        fn base_params(&self) -> serde_json::Value { serde_json::Value::Null }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> chem_core::step::StepRunResult {
            chem_core::step::StepRunResult::Success { outputs: vec![] }
        }
        fn kind(&self) -> chem_core::step::StepKind { chem_core::step::StepKind::Transform }
        fn name(&self) -> &str { self.id() }
    }
    let steps: Vec<Box<dyn chem_core::step::StepDefinition>> = vec![Box::new(DemoStep)];
    let def = chem_core::build_flow_definition(&["f5_step"], steps);
    let instance = repo.load(flow_id, &events, &def);
    println!("[F5] replay lowlevel completed? {} (steps={})", instance.completed, def.len());
    drop(store);
}
