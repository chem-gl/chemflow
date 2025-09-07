//! Tests de integración F4 (pipeline Acquire→Compute)
//!
//! Nota: No se ejecutan automáticamente aquí por petición del usuario. Este
//! archivo queda listo para ser corrido con cargo test cuando se desee.

use chem_core::FlowEngine;

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

    // Segunda corrida (nuevo engine en memoria)
    let mut engine2 = FlowEngine::new().firstStep(AcquireMoleculesStep::new())
                                       .add_step(ComputePropertiesStep::new())
                                       .build();
    engine2.set_name("basic_acquire_compute");
    let _ = engine2.run_to_end().expect("run ok");
    let fp2 = engine2.flow_fingerprint().expect("fp2");
    let variants2 = engine2.event_variants().unwrap_or_default();

    assert_eq!(fp1, fp2, "Fingerprint debe ser reproducible");
    assert_eq!(variants1, variants2, "Secuencia de eventos debe coincidir");
}
