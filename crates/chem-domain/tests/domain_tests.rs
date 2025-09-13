use chem_domain::{DomainError, FamilyProperty, MolecularProperty, Molecule, MoleculeFamily};
use serde_json::json;

#[test]
fn test_molecular_property_list_equality() -> Result<(), DomainError> {
    // Crear una molécula de ejemplo
    let mol = Molecule::from_smiles("CCO")?;

    // Metadata y dos propiedades idénticas
    let metadata = json!({"source": "integration_test", "method": "test"});
    let prop1 = MolecularProperty::new(&mol, "logP", 1.23_f64, Some("high".to_string()), true, metadata.clone())?;
    let prop2 = MolecularProperty::new(&mol, "logP", 1.23_f64, Some("high".to_string()), true, metadata.clone())?;

    // Deben ser equivalentes y con integridad válida
    assert_eq!(prop1, prop2);
    assert!(prop1.verify_integrity()?);

    Ok(())
}

#[test]
fn test_family_property_and_family_operations() -> Result<(), DomainError> {
    // Crear dos moléculas y una familia
    let mol1 = Molecule::from_smiles("CCO")?;
    let mol2 = Molecule::from_smiles("CCN")?;
    let provenance = json!({"source": "integration_test"});
    let family = MoleculeFamily::new(vec![mol1.clone(), mol2.clone()], provenance)?;

    // Helpers len/contains añadidos
    assert_eq!(family.len(), 2);
    assert!(family.contains(mol1.inchikey()));

    // Crear propiedades de familia idénticas
    let metadata = json!({"calculation_method": "test"});
    let fprop1 = FamilyProperty::new(&family,
                                     "average_logP",
                                     2.5_f64,
                                     Some("high".to_string()),
                                     true,
                                     metadata.clone())?;
    let fprop2 = FamilyProperty::new(&family,
                                     "average_logP",
                                     2.5_f64,
                                     Some("high".to_string()),
                                     true,
                                     metadata.clone())?;

    assert_eq!(fprop1, fprop2);
    assert!(fprop1.verify_integrity()?);

    // Intentar agregar una molécula duplicada debe fallar
    let add_result = family.add_molecule(mol1.clone());
    assert!(add_result.is_err());

    // Remover una molécula debe producir una nueva familia con tamaño reducido
    let reduced = family.remove_molecule(mol2.inchikey())?;
    assert_eq!(reduced.len(), 1);

    // Intentar eliminar la última molécula debe fallar
    let final_inchikey = reduced.molecules()[0].inchikey().to_string();
    let remove_last = reduced.remove_molecule(&final_inchikey);
    assert!(remove_last.is_err());

    Ok(())
}
