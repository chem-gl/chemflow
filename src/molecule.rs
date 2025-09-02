//! Entidad básica `Molecule` que representa una molécula individual.
//! Contiene identificadores químicos comunes (InChIKey / SMILES / InChI) y
//! opcionalmente un nombre común. Este módulo es deliberadamente mínimo para
//! permitir ampliaciones futuras (anotaciones, índices, subestructuras, etc.).
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Molecule {
    /// InChIKey único empleado como clave primaria lógica.
    pub inchikey: String,  // Clave primaria
    /// Representación SMILES.
    pub smiles: String,
    /// Cadena InChI (puede ser derivada de la estructura real en futuras versiones).
    pub inchi: String,
    /// Nombre común opcional.
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
    
    /// Construye una molécula a partir de una cadena SMILES.
    /// (Placeholder: la validación real química se debe implementar más adelante).
    /// Retorna error sólo si la validación futura lo requiere.
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

