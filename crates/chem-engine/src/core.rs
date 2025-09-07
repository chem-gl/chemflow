use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use serde::Deserialize;
use std::ffi::CString;
use std::sync::OnceLock;

static RDKIT_MODULE: OnceLock<Py<PyModule>> = OnceLock::new();
pub fn init_python() -> PyResult<()> {
    Python::attach(|py| {
        let code = CString::new(include_str!("../python/rdkit_wrapper.py"))?;
        let module = PyModule::from_code(py, code.as_c_str(), c_str!("rdkit_wrapper.py"), c_str!("rdkit_wrapper"))?;
        // Guardamos el módulo en el OnceLock como Py<PyModule>
        RDKIT_MODULE.set(module.unbind()).ok();
        Ok(())
    })
}

fn get_module(py: Python<'_>) -> PyResult<Py<PyModule>> {
    RDKIT_MODULE.get().map(|module| module.clone_ref(py)).ok_or_else(|| {
                                                             PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "init_python() debe llamarse antes de get_molecule()"
        )
                                                         })
}

#[derive(Debug, Deserialize)]
pub struct Molecule {
    pub smiles: String,
    pub inchi: String,
    pub inchikey: String,
    pub num_atoms: u32,
    pub mol_weight: f64,
    pub mol_formula: String,
}

pub fn get_molecule(smiles: &str) -> PyResult<Molecule> {
    Python::attach(|py| {
        let rdkit_py = get_module(py)?;
        let rdkit = rdkit_py.bind(py);
        let binding = rdkit.getattr("molecule_info")?.call1((smiles,))?;
        let info = binding.downcast::<PyDict>()?;
        let json_str: String = py.import("json")?.call_method1("dumps", (info,))?.extract()?;
        let molecule: Molecule = serde_json::from_str(&json_str).map_err(|e| {
                                     PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Deserialization error: {}", e))
                                 })?;
        Ok(molecule)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_molecule_export() {
        let m = Molecule { smiles: "".to_string(),
                           inchi: "".to_string(),
                           inchikey: "".to_string(),
                           num_atoms: 0,
                           mol_weight: 0.0,
                           mol_formula: "".to_string() };
        assert_eq!(m.smiles, "");
        assert_eq!(m.num_atoms, 0);
    }
    #[test]
    fn test_get_molecule() {
        init_python().expect("Fallo al inicializar Python/RDKit");
        let smiles = "CCO"; // Etanol
        let mol = get_molecule(smiles).expect("Fallo al obtener la molécula");
        assert_eq!(mol.smiles, "CCO");
        assert_eq!(mol.num_atoms, 3);
        assert!((mol.mol_weight - 46.07).abs() < 0.1); // Peso molecular
                                                       // aproximado
    }
}
