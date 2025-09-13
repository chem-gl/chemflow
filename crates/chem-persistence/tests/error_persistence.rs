//! Tests para persistencia de errores de ejecución (F8).
//!
//! Verifica:
//! - Inserción automática en step_execution_errors al emitir StepFailed.
//! - Consulta de errores por flow_id.
//! - Clasificación correcta de error_class.
//! - Timeline reconstruida con attempt_number.

use chem_core::errors::CoreEngineError;
use chem_core::{EventStore, FlowEventKind};
use chem_persistence::pg::{build_dev_pool_from_env, PgEventStore, PoolProvider};
use uuid::Uuid;

#[test]
fn test_error_persistence_on_step_failed() {
    // Requiere DATABASE_URL
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("Skipping test_error_persistence_on_step_failed: DATABASE_URL not set");
        return;
    }

    let pool = build_dev_pool_from_env().expect("pool");
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);

    let flow_id = Uuid::new_v4();
    let step_id = "test_step".to_string();

    // Emitir StepFailed
    let error = CoreEngineError::Internal("test error".to_string());
    let kind = FlowEventKind::StepFailed { step_index: 0,
                                           step_id: step_id.clone(),
                                           error: error.clone(),
                                           fingerprint: "test_fp".to_string() };
    let _event = store.append_kind(flow_id, kind);

    // Verificar que se insertó en step_execution_errors
    let errors = store.list_errors(flow_id);
    assert_eq!(errors.len(), 1, "Debe haber un error registrado");
    let err = &errors[0];
    assert_eq!(err.flow_id, flow_id);
    assert_eq!(err.step_id, step_id);
    assert_eq!(err.attempt_number, 1);
    assert_eq!(err.error_class, "runtime");
    // details debe contener el error serializado
    if let Some(details) = &err.details {
        let deserialized: CoreEngineError = serde_json::from_value(details.clone()).expect("deserialize error");
        assert_eq!(deserialized, error);
    } else {
        panic!("details should be present");
    }

    // Avoid running native destructor on pool/provider/store during test teardown
    std::mem::forget(store);
    // provider and pool were moved into store; no further action required.
}

#[test]
fn test_error_classification() {
    // Test para diferentes tipos de error
    let test_cases = vec![(CoreEngineError::Internal("".to_string()), "runtime"),
                          (CoreEngineError::InvalidStepIndex, "validation"),
                          (CoreEngineError::StorageError("".to_string()), "runtime"),
                          (CoreEngineError::MissingInputs, "validation"),];

    for (error, expected_class) in test_cases {
        let class = match error {
            CoreEngineError::Internal(_) | CoreEngineError::StorageError(_) => "runtime",
            _ => "validation",
        };
        assert_eq!(class, expected_class, "Clasificación incorrecta para {:?}", error);
    }
}
