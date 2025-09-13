use chem_domain::Molecule;
use chem_core::step::StepKind;
use chem_core::FlowEngine;
use chem_core::{typed_artifact, typed_step};
use chem_adapters::steps::acquire::AcquireMoleculesStep;
use chem_adapters::steps::compute::ComputePropertiesStep;
use chem_adapters::encoder::{DomainArtifactEncoder, SimpleDomainEncoder};
use chem_domain::MoleculeFamily;

// --------------------
// Artifactos tipados
// --------------------
typed_artifact!(TextOut { text: String });
typed_artifact!(CharsOut { chars: Vec<char> });
typed_artifact!(CountOut { count: usize });

// --------------------
// Steps tipados
// --------------------

// Step fuente que genera texto
typed_step! {
    source TextSource {
        id: "text_source",
        output: TextOut,
        params: (),
        fields { message: String },
        run(me, _p) {
            TextOut { text: me.message.clone(), schema_version: 1 }
        }
    }
}

// Step que transforma texto en caracteres
typed_step! {
    step TextToChars {
        id: "text_to_chars",
        kind: StepKind::Transform,
        input: TextOut,
        output: CharsOut,
        params: (),
        run(_self, inp, _p) {
            let chars: Vec<char> = inp.text.chars().collect();
            CharsOut { chars, schema_version: 1 }
        }
    }
}

// Step que cuenta caracteres
typed_step! {
    step CountChars {
        id: "count_chars",
        kind: StepKind::Sink,
        input: CharsOut,
        output: CountOut,
        params: (),
        run(_self, inp, _p) {
            println!("Procesando caracteres: {:?}", inp.chars);
            CountOut { count: inp.chars.len(), schema_version: 1 }
        }
    }
}

fn main() {
    println!("🚀 Iniciando ChemFlow - Demo de Flujo Tipado");
    println!("==============================================");

    // Cargar variables de entorno desde .env si existe
    let _ = dotenvy::dotenv();

    // -------------------- DEMO 1: Flujo Básico con Steps Tipados --------------------

    println!("\n📝 Demo 1: Flujo básico con steps tipados");
    println!("------------------------------------------");

    // Crear el engine usando el patrón builder
    let mut engine = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(TextSource::new("Hola ChemFlow!".to_string()))
    .add_step(TextToChars::new())
    .add_step(CountChars::new())
    .build();

    // Ejecutar el flujo completo
    match engine.run_to_completion() {
        Ok(flow_id) => {
            println!("✅ Flujo completado exitosamente!");
            println!("   Flow ID: {}", flow_id);

            // Obtener eventos del flujo
            if let Some(events) = engine.events() {
                println!("   Número de eventos: {}", events.len());

                // Mostrar secuencia de eventos
                let variants: Vec<String> = events.iter()
                    .map(|e| match &e.kind {
                        chem_core::FlowEventKind::FlowInitialized { .. } => "I",
                        chem_core::FlowEventKind::StepStarted { .. } => "S",
                        chem_core::FlowEventKind::StepFinished { .. } => "F",
                        chem_core::FlowEventKind::StepFailed { .. } => "X",
                        chem_core::FlowEventKind::StepSignal { .. } => "G",
                        chem_core::FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
                        chem_core::FlowEventKind::RetryScheduled { .. } => "R",
                        chem_core::FlowEventKind::BranchCreated { .. } => "B",
                        chem_core::FlowEventKind::UserInteractionRequested { .. } => "U",
                        chem_core::FlowEventKind::UserInteractionProvided { .. } => "V",
                        chem_core::FlowEventKind::FlowCompleted { .. } => "C",
                    })
                    .map(|s| s.to_string())
                    .collect();

                println!("   Secuencia de eventos: {}", variants.join(" → "));
            }

            // Obtener fingerprint del flujo
            if let Some(fp) = engine.flow_fingerprint() {
                println!("   Flow fingerprint: {}", fp);
            }
        }
        Err(e) => {
            println!("❌ Error ejecutando el flujo: {:?}", e);
        }
    }

    // -------------------- DEMO 2: Flujo Químico (F4) --------------------

    println!("\n🧪 Demo 2: Flujo químico con adquisición y computación");
    println!("-----------------------------------------------------");

    // Crear moléculas de ejemplo
    let smiles_benzene = "C1=CC=CC=C1"; // Benceno
    let smiles_ethanol = "CCO"; // Etanol

    // Crear moléculas y manejar posibles errores
    let molecule1 = Molecule::from_smiles(smiles_benzene).expect("Error al crear la molécula 1");
    let molecule2 = Molecule::from_smiles(smiles_ethanol).expect("Error al crear la molécula 2");

    println!("   Molécula 1: {} (InChI: {})", molecule1, molecule1.inchi());
    println!("   Molécula 2: {} (InChI: {})", molecule2, molecule2.inchi());

    // F4: uso del encoder dominio → artifact neutral (molecule y family)
    let encoder = SimpleDomainEncoder::default();
    let mol_art = encoder.encode_molecule(&molecule1);
    println!("   Artifact molécula: kind={:?}, hash={}",
             mol_art.kind,
             mol_art.hash);

    // Construcción de familia determinista mínima con provenance estable
    let provenance = serde_json::json!({ "source": "main_demo", "version": 1 });
    let family = MoleculeFamily::new(vec![molecule1, molecule2], provenance).expect("family ok");
    println!("   Family hash: {}", family.family_hash());

    let fam_art = encoder.encode_family(&family);
    println!("   Artifact familia: kind={:?}, hash={}",
             fam_art.kind,
             fam_art.hash);

    // Construir y ejecutar el flujo químico
    let mut engine4 = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(AcquireMoleculesStep::new())
    .add_step(ComputePropertiesStep::new())
    .build();

    match engine4.run_to_completion() {
        Ok(flow_id) => {
            println!("✅ Flujo químico completado!");
            println!("   Flow ID: {}", flow_id);

            if let Some(fp) = engine4.flow_fingerprint() {
                println!("   Flow fingerprint: {}", fp);
            }

            // Obtener eventos
            if let Some(events) = engine4.events() {
                let variants: Vec<String> = events.iter()
                    .map(|e| match &e.kind {
                        chem_core::FlowEventKind::FlowInitialized { .. } => "I",
                        chem_core::FlowEventKind::StepStarted { .. } => "S",
                        chem_core::FlowEventKind::StepFinished { .. } => "F",
                        chem_core::FlowEventKind::StepFailed { .. } => "X",
                        chem_core::FlowEventKind::StepSignal { .. } => "G",
                        chem_core::FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
                        chem_core::FlowEventKind::RetryScheduled { .. } => "R",
                        chem_core::FlowEventKind::BranchCreated { .. } => "B",
                        chem_core::FlowEventKind::UserInteractionRequested { .. } => "U",
                        chem_core::FlowEventKind::UserInteractionProvided { .. } => "V",
                        chem_core::FlowEventKind::FlowCompleted { .. } => "C",
                    })
                    .map(|s| s.to_string())
                    .collect();

                println!("   Secuencia de eventos: {}", variants.join(" → "));
            }
        }
        Err(e) => {
            println!("❌ Error en flujo químico: {:?}", e);
        }
    }

    // -------------------- DEMO 3: Flujo Paso a Paso --------------------

    println!("\n🔄 Demo 3: Ejecución paso a paso");
    println!("-------------------------------");

    let mut engine_step = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(TextSource::new("Paso a paso".to_string()))
    .add_step(TextToChars::new())
    .add_step(CountChars::new())
    .build();

    println!("   Ejecutando paso a paso...");

    // Ejecutar paso por paso
    for step_num in 1..=3 {
        match engine_step.next() {
            Ok(_) => {
                println!("   ✅ Paso {} completado", step_num);
            }
            Err(e) => {
                println!("   ❌ Error en paso {}: {:?}", step_num, e);
                break;
            }
        }
    }

    // Verificar si el flujo se completó
    if let Some(events) = engine_step.events() {
        let completed = events.iter()
            .any(|e| matches!(e.kind, chem_core::FlowEventKind::FlowCompleted { .. }));
        println!("   Flujo completado: {}", completed);
    }

    // -------------------- DEMO 4: Determinismo --------------------

    println!("\n🔄 Demo 4: Verificación de determinismo");
    println!("--------------------------------------");

    // Ejecutar el mismo flujo dos veces
    let mut engine1 = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(TextSource::new("Determinismo".to_string()))
    .add_step(TextToChars::new())
    .add_step(CountChars::new())
    .build();

    let mut engine2 = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(TextSource::new("Determinismo".to_string()))
    .add_step(TextToChars::new())
    .add_step(CountChars::new())
    .build();

    match (engine1.run_to_completion(), engine2.run_to_completion()) {
        (Ok(id1), Ok(id2)) => {
            let fp1 = engine1.flow_fingerprint().unwrap_or_default();
            let fp2 = engine2.flow_fingerprint().unwrap_or_default();

            println!("   Flow 1 ID: {}", id1);
            println!("   Flow 2 ID: {}", id2);
            println!("   Flow 1 fingerprint: {}", fp1);
            println!("   Flow 2 fingerprint: {}", fp2);
            println!("   Fingerprints iguales: {}", fp1 == fp2);

            if fp1 == fp2 {
                println!("   ✅ ¡Determinismo verificado!");
            } else {
                println!("   ⚠️  Fingerprints diferentes - posible no determinismo");
            }
        }
        _ => {
            println!("   ❌ Error ejecutando flujos para determinismo");
        }
    }

    println!("\n🎉 ¡Demo completado exitosamente!");
    println!("==================================");

    // -------------------- DEMO 5: Persistencia en Postgres y Branching --------------------

    if std::env::var("DATABASE_URL").is_ok() {
        println!("\n🗄️ Demo 5: Persistencia en Postgres y branching (si DATABASE_URL está presente)");
        println!("------------------------------------------------------------------");

        use chem_persistence::{build_dev_pool_from_env, PoolProvider, PgEventStore, PgFlowRepository};

        let pool = match build_dev_pool_from_env() {
            Ok(p) => p,
            Err(e) => {
                println!("   ⚠️  No se pudo construir pool PG: {:?} - saltando demo PG", e);
                return;
            }
        };
        let provider = PoolProvider { pool };
        let store = PgEventStore::new(provider);
        let repo = PgFlowRepository::new();

        let mut pg_engine = FlowEngine::new_with_stores(store, repo);

        // Reuse a tiny definition: seed -> next (use simple typed steps declared above)
        let def = chem_core::repo::build_flow_definition(&["text_source", "text_to_chars", "count_chars"],
                                                        vec![Box::new(TextSource::new("pg demo".to_string())),
                                                             Box::new(TextToChars::new()),
                                                             Box::new(CountChars::new())]);

        let flow_id = uuid::Uuid::new_v4();
        println!("   Creando flow en Postgres: {}", flow_id);
        if let Err(e) = pg_engine.next_with(flow_id, &def) {
            println!("   ❌ Error ejecutando primer step en PG: {:?}", e);
        } else {
            println!("   ✅ Primer step ejecutado y persistido en PG");
        }

        // Crear rama a partir del primer step
        if let Ok(branch_id) = pg_engine.branch(flow_id, &def, "text_source", Some("demo-divergence".to_string())) {
            println!("   ✅ Rama creada: {}", branch_id);
            // Continuar la rama
            if let Err(e) = pg_engine.next_with(branch_id, &def) {
                println!("   ❌ Error ejecutando branch next: {:?}", e);
            } else {
                println!("   ✅ Branch avanzó y eventos persistidos");
            }
        } else {
            println!("   ⚠️  No se pudo crear la rama (ver logs)");
        }
    } else {
        println!("\nℹ️ DATABASE_URL no presente: se omite demo Postgres");
    }
}
