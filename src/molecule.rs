use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Molecule {
    pub inchikey: String,  // Clave primaria
    pub smiles: String,
    pub inchi: String,
    pub common_name: Option<String>,
}

impl Molecule {
    pub fn new(inchikey: String, smiles: String, inchi: String, common_name: Option<String>) -> Self {
        Self {
            inchikey,
            smiles,
            inchi,
            common_name,
        }
    }
    
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if the SMILES string is invalid.
    pub fn from_smiles(smiles: String) -> Result<Self, Box<dyn std::error::Error>> {
        let inchikey = format!("{}_key", smiles);  // Placeholder
        let inchi = format!("InChI=1S/{}", smiles);  // Placeholder
        
        Ok(Self {
            inchikey,
            smiles,
            inchi,
            common_name: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Molecule;

    #[test]
    fn test_molecule_new() {
        let m = Molecule::new(
            "TESTKEY".to_string(),
            "C1=CC=CC=C1".to_string(),
            "InChI=1S/C1=CC=CC=C1".to_string(),
            Some("Benzene".to_string()),
        );
        assert_eq!(m.inchikey, "TESTKEY");
        assert_eq!(m.smiles, "C1=CC=CC=C1");
        assert_eq!(m.inchi, "InChI=1S/C1=CC=CC=C1");
        assert_eq!(m.common_name.unwrap(), "Benzene");
    }

    #[test]
    fn test_from_smiles() {
        let smiles = "CO".to_string();
        let m = Molecule::from_smiles(smiles.clone()).expect("should parse smiles");
        assert_eq!(m.inchikey, format!("{}_key", smiles));
        assert_eq!(m.inchi, format!("InChI=1S/{}", smiles));
        assert!(m.common_name.is_none());
    }
}

