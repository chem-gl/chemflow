// Importing Molecule from the chem-domain crate
use chem_domain::Molecule;

// --- Ejemplo F2: Steps tipados para un flujo determinista ---
use chem_core::step::StepKind;
use chem_core::FlowEngine;
use chem_core::{typed_artifact, typed_step};
// Helper público: crea un builder de FlowEngine con repositorio (Postgres)
// para usar de forma concisa como FlowEngine::new().firstStep(...)
use chem_persistence::{PgEventStore, PgFlowRepository, PoolProvider};
// F4: Steps y artefacto de chem-adapters
use chem_adapters::artifacts::FamilyPropertiesArtifact;
use chem_adapters::steps::acquire::AcquireMoleculesStep;
use chem_adapters::steps::compute::ComputePropertiesStep;
use chem_adapters::encoder::{DomainArtifactEncoder, SimpleDomainEncoder};
use chem_domain::MoleculeFamily;
use serde_json::to_string_pretty;

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
    // validacion del flujo 3
    maybe_run_pg_demo();
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
    if let Err(e) = pg_persistence_demo::run() {
        eprintln!("[PG DEMO] Error (basic): {e:?}");
    }
    if let Err(e) = pg_persistence_demo::run_replay_parity() {
        eprintln!("[PG DEMO] Error (replay): {e:?}");
    }
}
