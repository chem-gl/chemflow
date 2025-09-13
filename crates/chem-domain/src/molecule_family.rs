// molecule_family.rs
use crate::{DomainError, Molecule};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;

/// Representa una colección inmutable de moléculas relacionadas con metadatos
/// y verificación de integridad mediante hash. Ideal para agrupar moléculas
/// con propiedades estructurales o funcionales similares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeFamily {
    id: Uuid,
    name: Option<String>,
    description: Option<String>,
    family_hash: String,
    provenance: serde_json::Value,
    frozen: bool,
    molecules: Vec<Molecule>,
}

impl MoleculeFamily {
    /// Crea una nueva familia molecular inmutable a partir de un iterador de
    /// moléculas
    ///
    /// # Argumentos
    /// * `molecules` - Iterador de moléculas para incluir en la familia
    /// * `provenance` - Metadatos sobre el origen y creación de la familia
    ///
    /// # Errores
    /// Retorna `DomainError::ValidationError` si hay moléculas duplicadas en la
    /// familia
    pub fn new<I>(molecules: I, provenance: serde_json::Value) -> Result<Self, DomainError>
        where I: IntoIterator<Item = Molecule>
    {
        let molecules: Vec<Molecule> = molecules.into_iter().collect();
        // Validar que la familia no esté vacía
        if molecules.is_empty() {
            return Err(DomainError::ValidationError("Una familia molecular no puede estar vacía".to_string()));
        }
        // Validar duplicados por InChIKey
        let mut seen_inchikeys = HashSet::new();
        for molecule in &molecules {
            if !seen_inchikeys.insert(molecule.inchikey().to_owned()) {
                return Err(DomainError::ValidationError(format!("Molécula duplicada en familia: {}", molecule.inchikey())));
            }
        }
        // Generar hash basado en la secuencia de InChIKeys de las moléculas
        let family_hash = Self::calculate_family_hash(&molecules);
        Ok(MoleculeFamily { id: Uuid::new_v4(),
                            name: None,
                            description: None,
                            family_hash,
                            provenance,
                            frozen: true, // Las familias son inmutables por defecto
                            molecules })
    }

    /// Calcula el hash de la familia basado en los InChIKeys de las moléculas
    fn calculate_family_hash(molecules: &[Molecule]) -> String {
        let mut hasher = Sha256::new();

        // Incluir todos los InChIKeys en el hash para verificación de integridad
        for molecule in molecules {
            hasher.update(molecule.inchikey().as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Crea una nueva instancia con nombre modificado
    pub fn with_name(&self, name: impl Into<String>) -> Result<Self, DomainError> {
        let mut new_family = self.clone();
        new_family.name = Some(name.into());
        new_family.id = Uuid::new_v4(); // Nuevo ID para la nueva versión
        Ok(new_family)
    }

    /// Crea una nueva instancia con descripción modificada
    pub fn with_description(&self, description: impl Into<String>) -> Result<Self, DomainError> {
        let mut new_family = self.clone();
        new_family.description = Some(description.into());
        new_family.id = Uuid::new_v4(); // Nuevo ID para la nueva versión
        Ok(new_family)
    }

    /// Agrega una molécula a la familia, creando una nueva instancia
    ///
    /// # Errores
    /// Retorna `DomainError::ValidationError` si la molécula ya existe en la
    /// familia
    pub fn add_molecule(&self, molecule: Molecule) -> Result<Self, DomainError> {
        // Verificar si la molécula ya existe en la familia
        if self.molecules.iter().any(|m| m.inchikey() == molecule.inchikey()) {
            return Err(DomainError::ValidationError(format!("Molécula ya existe en la familia: {}", molecule.inchikey())));
        }

        // Crear nueva lista de moléculas
        let mut new_molecules = self.molecules.clone();
        new_molecules.push(molecule);

        // Calcular nuevo hash
        let family_hash = Self::calculate_family_hash(&new_molecules);

        Ok(MoleculeFamily { id: Uuid::new_v4(),
                            name: self.name.clone(),
                            description: self.description.clone(),
                            family_hash,
                            provenance: self.provenance.clone(),
                            frozen: true,
                            molecules: new_molecules })
    }

    /// Elimina una molécula de la familia por su InChIKey, creando una nueva
    /// instancia
    ///
    /// # Argumentos
    /// * `inchikey` - InChIKey de la molécula a eliminar
    pub fn remove_molecule(&self, inchikey: &str) -> Result<Self, DomainError> {
        // Filtrar la molécula a eliminar
        let new_molecules: Vec<Molecule> = self.molecules.iter().filter(|m| m.inchikey() != inchikey).cloned().collect();

        // Validar que la familia no quede vacía
        if new_molecules.is_empty() {
            return Err(DomainError::ValidationError("No se puede eliminar la última molécula de la familia".to_string()));
        }

        // Calcular nuevo hash
        let family_hash = Self::calculate_family_hash(&new_molecules);

        Ok(MoleculeFamily { id: Uuid::new_v4(),
                            name: self.name.clone(),
                            description: self.description.clone(),
                            family_hash,
                            provenance: self.provenance.clone(),
                            frozen: true,
                            molecules: new_molecules })
    }

    /// Verifica la integridad de la familia recalculando y comparando el hash
    pub fn verify_integrity(&self) -> bool {
        let calculated_hash = Self::calculate_family_hash(&self.molecules);
        calculated_hash == self.family_hash
    }

    // Getters
    /// Obtiene la lista inmutable de moléculas de la familia
    pub fn molecules(&self) -> &[Molecule] {
        &self.molecules
    }

    /// Indica cuántas moléculas contiene la familia
    pub fn len(&self) -> usize {
        self.molecules.len()
    }

    /// Indica si la familia está vacía
    pub fn is_empty(&self) -> bool {
        self.molecules.is_empty()
    }

    /// Indica si la familia contiene una molécula con el InChIKey dado
    pub fn contains(&self, inchikey: &str) -> bool {
        self.molecules.iter().any(|m| m.inchikey() == inchikey)
    }

    /// Obtiene el hash único que identifica la composición de la familia
    pub fn family_hash(&self) -> &str {
        &self.family_hash
    }

    /// Indica si la familia está congelada (inmutable)
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Obtiene el ID único de la familia
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Obtiene el nombre de la familia si está definido
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    /// Obtiene la descripción de la familia si está definida
    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    /// Obtiene los metadatos de procedencia de la familia
    pub fn provenance(&self) -> &serde_json::Value {
        &self.provenance
    }

    /// Compara si dos familias son equivalentes basándose en su hash
    pub fn is_equivalent(&self, other: &MoleculeFamily) -> bool {
        self.family_hash == other.family_hash
    }
}

// Implementación de IntoIterator para referencia
impl<'a> IntoIterator for &'a MoleculeFamily {
    type Item = &'a Molecule;
    type IntoIter = std::slice::Iter<'a, Molecule>;

    fn into_iter(self) -> Self::IntoIter {
        self.molecules.iter()
    }
}

// Implementación de IntoIterator para consumo
impl IntoIterator for MoleculeFamily {
    type Item = Molecule;
    type IntoIter = std::vec::IntoIter<Molecule>;

    fn into_iter(self) -> Self::IntoIter {
        self.molecules.into_iter()
    }
}

// Implementación de Display para formato legible
impl fmt::Display for MoleculeFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
               "MoleculeFamily(id: {}, name: {}, molecules: {})",
               self.id,
               self.name.as_deref().unwrap_or("sin nombre"),
               self.molecules.len())
    }
}

// Implementación de PartialEq basada en el hash de la familia
impl PartialEq for MoleculeFamily {
    fn eq(&self, other: &Self) -> bool {
        self.is_equivalent(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_molecule_family_creation() -> Result<(), DomainError> {
        let mol1 = Molecule::from_smiles("CCO")?;
        let mol2 = Molecule::from_smiles("CCN")?;

        let provenance = json!({"source": "test"});
        let family = MoleculeFamily::new(vec![mol1, mol2], provenance)?;

        assert_eq!(family.molecules().len(), 2);
        assert!(family.verify_integrity());
        Ok(())
    }

    #[test]
    fn test_molecule_family_duplicates() -> Result<(), DomainError> {
        let mol = Molecule::from_smiles("CCO")?;
        let provenance = json!({"source": "test"});

        let result = MoleculeFamily::new(vec![mol.clone(), mol], provenance);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_molecule_family_empty() {
        let provenance = json!({"source": "test"});
        let result = MoleculeFamily::new(Vec::<Molecule>::new(), provenance);
        assert!(result.is_err());
    }
}
