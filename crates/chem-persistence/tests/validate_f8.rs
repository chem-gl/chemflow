use std::path::Path;

use chem_core::errors::{classify_error, CoreEngineError, ErrorClass};
use chem_persistence::pg::ErrorRow;
use chem_persistence::schema::step_execution_errors;

#[test]
fn validate_f8_implementation_smoke() {
    // 1) Migración presente (path relativo al crate en tiempo de test)
    let mut mig = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    mig.push("migrations/0005_step_execution_errors/up.sql");
    assert!(mig.exists(),
            "migration 0005_step_execution_errors/up.sql must exist for F8 (checked: {:?})",
            mig);

    // 2) classify_error compiles and returns expected classification for Internal
    let e = CoreEngineError::Internal("x".to_string());
    let cls = classify_error(&e);
    assert_eq!(cls, ErrorClass::Runtime);

    // 3) ErrorRow type is exported and the Diesel table symbol exists (compile-time
    //    checks)
    let _maybe: Option<ErrorRow> = None;
    // referencing the Diesel table symbol ensures it was declared in schema.rs
    let _ = step_execution_errors::table;
}
