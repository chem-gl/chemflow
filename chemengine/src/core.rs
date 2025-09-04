use dotenvy::dotenv;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::env;

/// Inicializa Python/RDKit usando el archivo .env
pub fn init_python() -> PyResult<()> {
    dotenv().ok();
    let python_path =
        env::var("PYTHON_PATH").expect("❌ PYTHON_PATH no encontrado en .env. Ejecuta setup-python.sh primero.");
    env::set_var("PYTHON_SYS_EXECUTABLE", python_path);
    Ok(())
}

/// Calcula el peso molecular de un SMILES usando RDKit
pub fn mol_weight(smiles: &str) -> PyResult<f64> {
    Python::with_gil(|py| {
        let code = include_str!("../python/rdkit_wrapper.py");
        let rdkit = PyModule::from_code(py, code, "rdkit_wrapper.py", "rdkit_wrapper")?;
        let weight: f64 = rdkit.getattr("mol_weight")?.call1((smiles,))?.extract()?;
        Ok(weight)
    })
}
