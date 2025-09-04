use chem_domain::{DomainError, MolecularProperty, Molecule, MoleculeFamily};
use serde_json::json;
use std::collections::HashSet;

#[test]
fn test_molecule_new_valid() {
    let ok = Molecule::new("aaaaaaaaaaaaaa-bbbbbbbbbb-c", "smi", "inchi", json!({}));
    assert!(ok.is_ok());
    let m = ok.unwrap();
    assert_eq!(m.inchikey(), "AAAAAAAAAAAAAA-BBBBBBBBBB-C");
}

#[test]
fn test_molecule_new_invalid() {
    let err = Molecule::new("short", "", "", json!({}));
    assert!(matches!(err, Err(DomainError::ValidationError(_))));
}

#[test]
fn test_molecule_uniqueness_in_set() {
    let m1 = Molecule::new("aaaaaaaaaaaaaa-bbbbbbbbbb-c", "", "", json!({})).unwrap();
    let m2 = Molecule::new("AAAAAAAAAAAAAA-BBBBBBBBBB-C", "", "", json!({})).unwrap();
    let mut set: HashSet<String> = HashSet::new();
    assert!(set.insert(m1.inchikey().to_string()));
    assert!(!set.insert(m2.inchikey().to_string()));
}

#[test]
fn test_molecule_family_hash_and_freeze() {
    let keys = vec!["B", "A", "C"];
    let fam1 = MoleculeFamily::from_iter(keys.clone(), json!({}));
    let fam2 = MoleculeFamily::from_iter(keys.clone().into_iter().rev(), json!({}));
    assert_eq!(fam1.family_hash(), fam2.family_hash());
    assert!(fam1.is_frozen());
    assert_eq!(fam1.ordered_keys(), &vec!["A".to_string(), "B".to_string(), "C".to_string()]);
}

#[test]
fn test_molecular_property_hash_deterministic() {
    let m = Molecule::new("aaaaaaaaaaaaaa-bbbbbbbbbb-c", "", "", json!({})).unwrap();
    let val = json!({"a": 1});
    let prop1 = MolecularProperty::new(&m,
                                       "name",
                                       val.clone(),
                                       Some("u".to_string()),
                                       Some("q".to_string()),
                                       true,
                                       None);
    let prop2 = MolecularProperty::new(&m, "name", val, Some("u".to_string()), Some("q".to_string()), true, None);
    assert_eq!(prop1.value_hash(), prop2.value_hash());
}
