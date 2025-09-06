// Importing Molecule from the chem-domain crate
use chem_domain::Molecule;

// --- Ejemplo F2: Steps tipados para un flujo determinista ---
use chem_core::FlowEngine;
use chem_core::step::StepKind;
use chem_core::{typed_artifact, typed_step};

// --------------------
// Artifactos tipados (menos verboso con macros)
// --------------------
typed_artifact!(TextOut  { text: String });
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
    let molecule1 = Molecule::new_molecule_with_smiles(smiles_benzene)
        .expect("Error al crear la molécula 1");
    let molecule2 = Molecule::new_molecule_with_smiles(smiles_ethanol)
        .expect("Error al crear la molécula 2");

    // Imprimir detalles de las moléculas
    println!("Molecula 1: {}", molecule1);
    println!("InChI de Molecula 1: {}", molecule1.inchi());

    println!("Molecula 2: {}", molecule2);
    println!("InChI de Molecula 2: {}", molecule2.inchi());

 
    // Construir y ejecutar el flujo
    let mut engine = FlowEngine::new()
        .firstStep(SeedStep::new("HolaMundo".to_string()))
        .addStep(SplitStep::new())
        .addStep(ForwardStep::new())
        .addStep(PrintAndCountStep::new())
        .build();
    engine.set_name("demo_chars");
    // Ejecutar hasta completar el flujo
    engine.run_to_end().expect("run ok");

    // Revisar eventos emitidos (I, S, F, S, F, C)
    let variants = engine.event_variants().unwrap_or_default();
    println!("Secuencia de eventos F2: {:?}", variants);

    let events = engine.events().unwrap();
    let finished_count = events
        .iter()
        .filter(|e| matches!(e.kind, chem_core::FlowEventKind::StepFinished { .. }))
        .count();
    let completed = events
        .iter()
        .any(|e| matches!(e.kind, chem_core::FlowEventKind::FlowCompleted { .. }));
    assert_eq!(finished_count, 4, "Deben terminar 4 steps");
    assert!(completed, "Debe existir FlowCompleted al final del flujo");

    // Mostrar fingerprint agregado del flow (si ya está completado)
    let flow_fp = engine.flow_fingerprint().unwrap_or_default();
    println!("Flow fingerprint agregado: {}", flow_fp);

    // Recupera el último output tipado del step final y lo imprime
    if let Some(Ok(out)) = engine.last_step_output_typed::<CountOut>("print_count") {
        println!("Cantidad de letras: {}", out.inner.count);
    }

    println!("!Validación F2: OK (flujo ejecutado y completado determinísticamente)");
}
