use pyo3::PyErr;
use thiserror::Error;
pub mod core;
pub use core::Molecule;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Error inicializando Python/RDKit: {0}")]
    Init(PyErr),
    #[error("Error obteniendo molécula: {0}")]
    GetMolecule(PyErr),
}

pub struct ChemEngine {
    _private: (),
}

impl ChemEngine {
    pub fn init() -> Result<Self, EngineError> {
        core::init_python().map_err(EngineError::Init)?;
        Ok(Self { _private: () })
    }
    pub fn get_molecule(&self, smiles: &str) -> Result<Molecule, EngineError> {
        let molecule = core::get_molecule(smiles).map_err(EngineError::GetMolecule)?;
        Ok(molecule)
    }
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
}
