use chem_domain::{FamilyProperty, MolecularProperty, Molecule, MoleculeFamily};
use serde_json::json;

#[test]
fn test_molecular_property_list_equality() {
    // Two lists with the same molecules, values, quality, preferred flag, and
    // metadata should compare equal
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let meta = json!({"source": "test"});
    let props1 = vec![MolecularProperty::new(&mol, "prop", 1, None, true, meta.clone()),
                      MolecularProperty::new(&mol, "prop", 2, Some("high".to_string()), false, meta.clone()),];
    let props2 = vec![MolecularProperty::new(&mol, "prop", 1, None, true, meta.clone()),
                      MolecularProperty::new(&mol, "prop", 2, Some("high".to_string()), false, meta.clone()),];
    assert_eq!(props1.len(), props2.len());
    for (p1, p2) in props1.iter().zip(props2.iter()) {
        assert!(p1.compare(p2));
    }
}

#[test]
fn test_molecular_property_list_inequality_value() {
    // Different values should not compare equal
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let meta = json!({"source": "test"});
    let p1 = MolecularProperty::new(&mol, "prop", 1, None, false, meta.clone());
    let p2 = MolecularProperty::new(&mol, "prop", 2, None, false, meta.clone());
    assert!(!p1.compare(&p2));
}

#[test]
fn test_molecular_property_list_inequality_metadata() {
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let p1 = MolecularProperty::new(&mol, "prop", 1, None, false, json!({"a": 1}));
    let p2 = MolecularProperty::new(&mol, "prop", 1, None, false, json!({"a": 2}));
    assert!(!p1.compare(&p2));
}

#[test]
fn test_family_property_equality() {
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let provenance = json!({"source": "test"});
    let fam = MoleculeFamily::new(vec![mol.clone()], provenance.clone()).unwrap();
    let p1 = FamilyProperty::new(&fam, "prop", 1, None, true, provenance.clone());
    let p2 = FamilyProperty::new(&fam, "prop", 1, None, true, provenance.clone());
    assert!(p1.compare(&p2));
}

#[test]
fn test_family_property_inequality_value() {
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let provenance = json!({"source": "test"});
    let fam = MoleculeFamily::new(vec![mol.clone()], provenance.clone()).unwrap();
    let p1 = FamilyProperty::new(&fam, "prop", 1, None, false, provenance.clone());
    let p2 = FamilyProperty::new(&fam, "prop", 2, None, false, provenance.clone());
    assert!(!p1.compare(&p2));
}

#[test]
fn test_family_property_inequality_metadata() {
    let mol = Molecule::new_molecule_with_smiles("CCO").unwrap();
    let fam = MoleculeFamily::new(vec![mol.clone()], json!({"a": 1})).unwrap();
    let p1 = FamilyProperty::new(&fam, "prop", 1, None, false, json!({"a": 1}));
    let p2 = FamilyProperty::new(&fam, "prop", 1, None, false, json!({"a": 2}));
    assert!(!p1.compare(&p2));
}
