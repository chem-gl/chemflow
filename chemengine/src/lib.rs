pub mod rdkit;


pub struct ChemEngine;

impl ChemEngine {
    /// Inicializa Python/RDKit y devuelve una instancia de ChemEngine
    pub fn init() -> Result<Self, String> {
        rdkit::init_python().map_err(|e| format!("Error inicializando Python/RDKit: {:?}", e))?;
        Ok(Self {})
    }

    /// Calcula el peso molecular de un SMILES
    pub fn mol_weight(&self, smiles: &str) -> Result<f64, String> {
        rdkit::mol_weight(smiles)
            .map_err(|e| format!("Error calculando peso molecular: {:?}", e))
    }
}
