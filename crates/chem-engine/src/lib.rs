pub mod core;
pub use core::Molecule;

pub struct ChemEngine {
    _private: (),
}

impl ChemEngine {
    pub fn init() -> Result<Self, String> {
        core::init_python()
            .map_err(|e| format!("Error inicializando Python/RDKit: {:?}", e))?;
        Ok(Self { _private: () })
    }
    pub fn get_molecule(&self, smiles: &str) -> Result<Molecule, String> {
        core::get_molecule(smiles)
            .map_err(|e| format!("Error obteniendo molécula: {:?}", e))
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_molecule_export() {
        let m = Molecule {
            smiles: "".to_string(),
            inchi: "".to_string(),
            inchikey: "".to_string(),
            num_atoms: 0,
            mol_weight: 0.0,
            mol_formula: "".to_string(),
        };
        assert_eq!(m.smiles, "");
        assert_eq!(m.num_atoms, 0);
    }
}
