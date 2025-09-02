use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;

use chemflow_rust::data::family::MoleculeFamily;
use chemflow_rust::database::repository::WorkflowExecutionRepository;
use chemflow_rust::providers::properties::implementations::generic_physchem::GenericPhysChemProvider;
use chemflow_rust::providers::properties::trait_properties::PropertiesProvider;
use chemflow_rust::workflow::manager::WorkflowManager;
use chemflow_rust::workflow::step::{FilterStep, MultiPropSpec, MultiPropertiesStep, WorkflowStep};
use chemflow_rust::workflow::step::MultiMoleculeAcquisitionStep;

// Simple helper to build an in-memory manager with only properties providers.
fn build_manager_with_physchem() -> WorkflowManager {
    let repo = WorkflowExecutionRepository::new(true);
    let mut props: HashMap<String, Box<dyn PropertiesProvider>> = HashMap::new();
    props.insert("generic_physchem".into(), Box::new(GenericPhysChemProvider::new()));
    WorkflowManager::new(repo, HashMap::new(), props, HashMap::new())
}

#[tokio::test]
async fn test_multi_properties_and_filter_branch_tree() {
    let mut manager = build_manager_with_physchem();

    // 1. Familia inicial vacía (sin moléculas basta para este mock provider)
    let mut fam = MoleculeFamily::new("Initial".into(), Some("Test multi props".into()));
    fam.parameters.insert("seed".into(), Value::Number(42.into()));
    let families = vec![fam];

    // 2. Step multi propiedades
    let step_multi = MultiPropertiesStep { id: Uuid::new_v4(),
                                           name: "multi_props".into(),
                                           description: "Calcula múltiples propiedades".into(),
                                           specs: vec![MultiPropSpec { provider: "generic_physchem".into(),
                                                                       property: "logp".into(),
                                                                       parameters: HashMap::new() }] };

    let out_multi = manager.execute_step(&step_multi, families, HashMap::new()).await.expect("multi props step");
    assert!(!out_multi.families.is_empty());
    // Verificamos que la propiedad agregada exista
    let fam_after = &out_multi.families[0];
    assert!(fam_after.get_property("logp").is_some(), "logp property missing after multi props step");
    assert!(out_multi.execution_info.parameter_hash.is_some());
    assert!(!out_multi.execution_info.providers_used.is_empty());

    // 3. Step de filtrado (debería clonar la familia creando una rama lógica)
    let filter = FilterStep { id: Uuid::new_v4(),
                              name: "filter_logp".into(),
                              description: "Filtra por logp".into(),
                              property: "logp".into(),
                              min: Some(1.0),
                              max: None };
    let out_filter = manager.execute_step(&filter, out_multi.families.clone(), HashMap::new()).await.expect("filter step");
    assert!(!out_filter.families.is_empty(), "Filter should produce cloned families");
    // La nueva familia debe tener parámetro filtered_from
    assert!(out_filter.families.iter().any(|f| f.parameters.get("filtered_from").is_some()));

    // 4. Branch tree & integrity checks
    let repo = manager.repository();
    let root_id = manager.root_execution_id();
    let tree = repo.build_branch_tree(root_id).await; // use method to avoid dead code warnings
    assert!(tree.is_array());

    // Verificar integridad del primer step
    let first_step_id = out_multi.execution_info.step_id;
    let integrity = repo.verify_execution_integrity(first_step_id).await;
    assert_eq!(integrity, Some(true));

    // 5. Export report (in-memory will still produce structure)
    let report = repo.export_workflow_report(root_id).await;
    assert!(report.get("steps").is_some());
}

#[tokio::test]
async fn test_multi_molecule_acquisition_step() {
    // Mock two molecule providers using existing test provider (register twice with different keys)
    use chemflow_rust::providers::molecule::implementations::test_provider::TestMoleculeProvider;
    let repo = WorkflowExecutionRepository::new(true);
    let mut mols: HashMap<String, Box<dyn chemflow_rust::providers::molecule::traitmolecule::MoleculeProvider>> = HashMap::new();
    mols.insert("prov_a".into(), Box::new(TestMoleculeProvider::new()));
    mols.insert("prov_b".into(), Box::new(TestMoleculeProvider::new()));
    let _manager = WorkflowManager::new(repo, HashMap::new(), HashMap::new(), HashMap::new()); // not used; ensures constructor remains covered
    // Step
    let multi = MultiMoleculeAcquisitionStep { id: Uuid::new_v4(), name: "multi_acq".into(), description: "multi provider acquisition".into(), provider_names: vec!["prov_a".into(), "prov_b".into()], parameters_per_provider: HashMap::new() };
    // Execute directly (bypassing manager to test step logic)
    let out = multi.execute(chemflow_rust::workflow::step::StepInput { families: Vec::new(), parameters: HashMap::new() }, &mols, &HashMap::new(), &HashMap::new()).await.unwrap();
    assert_eq!(out.families.len(), 1);
    assert!(!out.families[0].molecules.is_empty());
    assert!(out.execution_info.providers_used.len() >= 2);
}
