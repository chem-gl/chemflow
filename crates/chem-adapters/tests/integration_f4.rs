//! Tests de integración F4 (pipeline Acquire→Compute)
//!
//! Nota: No se ejecutan automáticamente aquí por petición del usuario. Este
//! archivo queda listo para ser corrido con cargo test cuando se desee.

use chem_core::FlowEngine;

use chem_adapters::artifacts::FamilyPropertiesArtifact;
use chem_adapters::steps::acquire::AcquireMoleculesStep;
use chem_adapters::steps::compute::ComputePropertiesStep;

#[test]
fn pipeline_acquire_compute_deterministic() {
    // Construir engine en memoria con steps tipados
    let mut engine = FlowEngine::new().firstStep(AcquireMoleculesStep::new())
                                      .add_step(ComputePropertiesStep::new())
                                      .build();
    engine.set_name("basic_acquire_compute");

    // Primera corrida
    let _ = engine.run_to_end().expect("run ok");
    let fp1 = engine.flow_fingerprint().expect("fp1");
    let variants1 = engine.event_variants().unwrap_or_default();
    // Afirmar N out == 3 (dataset synthetic_v1 tiene 3 moléculas)
    if let Some(Ok(out1)) = engine.last_step_output_typed::<FamilyPropertiesArtifact>("compute_properties") {
        assert_eq!(out1.inner.items.len(), 3, "Debe haber 3 propiedades (una por molécula)");
    } else {
        panic!("No se pudo recuperar el output tipado del step compute_properties");
    }

    // Segunda corrida (nuevo engine en memoria)
    let mut engine2 = FlowEngine::new().firstStep(AcquireMoleculesStep::new())
                                       .add_step(ComputePropertiesStep::new())
                                       .build();
    engine2.set_name("basic_acquire_compute");
    let _ = engine2.run_to_end().expect("run ok");
    let fp2 = engine2.flow_fingerprint().expect("fp2");
    let variants2 = engine2.event_variants().unwrap_or_default();
    if let Some(Ok(out2)) = engine2.last_step_output_typed::<FamilyPropertiesArtifact>("compute_properties") {
        assert_eq!(out2.inner.items.len(), 3, "Debe haber 3 propiedades (una por molécula)");
    } else {
        panic!("No se pudo recuperar el output tipado del step compute_properties (segunda corrida)");
    }

    assert_eq!(fp1, fp2, "Fingerprint debe ser reproducible");
    assert_eq!(variants1, variants2, "Secuencia de eventos debe coincidir");

    // Tercera corrida (nuevo engine en memoria)
    let mut engine3 = FlowEngine::new().firstStep(AcquireMoleculesStep::new())
                                       .add_step(ComputePropertiesStep::new())
                                       .build();
    engine3.set_name("basic_acquire_compute");
    let _ = engine3.run_to_end().expect("run ok");
    let fp3 = engine3.flow_fingerprint().expect("fp3");
    let variants3 = engine3.event_variants().unwrap_or_default();
    if let Some(Ok(out3)) = engine3.last_step_output_typed::<FamilyPropertiesArtifact>("compute_properties") {
        assert_eq!(out3.inner.items.len(), 3, "Debe haber 3 propiedades (una por molécula)");
    } else {
        panic!("No se pudo recuperar el output tipado del step compute_properties (tercera corrida)");
    }

    assert_eq!(fp1, fp3, "Fingerprint debe ser reproducible (1 vs 3)");
    assert_eq!(variants1, variants3, "Secuencia de eventos debe coincidir (1 vs 3)");
}
