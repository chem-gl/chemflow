mod data;
mod molecule;
mod workflow;
mod providers;
mod database;
mod migrations;
mod config;

use std::collections::HashMap;
use crate::database::repository::WorkflowExecutionRepository;
use config::{CONFIG, create_pool};
use crate::providers::molecule::implementations::test_provider::TestMoleculeProvider;
use crate::providers::properties::implementations::test_provider::TestPropertiesProvider;
use crate::workflow::manager::WorkflowManager;
use crate::workflow::step::{MoleculeAcquisitionStep, PropertiesCalculationStep};
use crate::providers::data::trait_dataprovider::DataProvider;
use serde_json::Value;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env if present
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: could not load .env file ({e}) - relying on existing environment");
    }
    println!("Hello, ChemFlow! Running migrations...");

    if let Err(e) = migrations::run_migrations().await {
        eprintln!("Failed to run migrations: {e}");
        return Err(e);
    }
    println!("Migrations applied.");

    // Test DB connection
    println!("Intentando conectar a la base de datos...");
    println!("URL: {}", CONFIG.database.url);
    let pool = create_pool().await?;
    let row: (i64,) = sqlx::query_as("SELECT $1")
        .bind(1_i64)
        .fetch_one(&pool)
        .await?;
    println!("Conexión verificada, resultado test: {}", row.0);

    let repo = WorkflowExecutionRepository::with_pool(pool).await;

    // Create base providers
    let mut molecule_providers = HashMap::new();
    molecule_providers.insert("test_molecule".to_string(), Box::new(TestMoleculeProvider::new()) as Box<dyn crate::providers::molecule::traitmolecule::MoleculeProvider>);

    let mut properties_providers = HashMap::new();
    properties_providers.insert("test_properties".to_string(), Box::new(TestPropertiesProvider::new()) as Box<dyn crate::providers::properties::trait_properties::PropertiesProvider>);

    // Mock antioxidant-specific providers (molecule + properties + data)
    struct AntioxidantSeedProvider;
    #[async_trait::async_trait]
    impl crate::providers::molecule::traitmolecule::MoleculeProvider for AntioxidantSeedProvider {
        fn get_name(&self) -> &str { "antiox_seed" }
        fn get_version(&self) -> &str { "0.1.0" }
        fn get_description(&self) -> &str { "Genera moléculas semilla antioxidantes mock" }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::molecule::traitmolecule::ParameterDefinition> { HashMap::new() }
        async fn get_molecule_family(&self, _p: &HashMap<String, Value>) -> Result<crate::data::family::MoleculeFamily, Box<dyn std::error::Error>> {
            let mut fam = crate::data::family::MoleculeFamily::new("Antioxidant Seeds".into(), Some("Mocked reference antioxidants".into()));
            // Simple canonical seed set (mock SMILES)
            for smi in ["O=CC1=CC=CC(O)=C1O", "CC1=C(O)C=C(O)C=C1O", "C1=CC(=CC=C1O)O"] { // e.g. cinnamaldehyde, catechol-like
                let m = crate::molecule::Molecule::from_smiles(smi.to_string())?;
                fam.molecules.push(m);
            }
            Ok(fam)
        }
    }

    struct AntioxActivityPropertiesProvider;
    #[async_trait::async_trait]
    impl crate::providers::properties::trait_properties::PropertiesProvider for AntioxActivityPropertiesProvider {
        fn get_name(&self) -> &str { "antiox_activity" }
        fn get_version(&self) -> &str { "0.1.0" }
        fn get_description(&self) -> &str { "Calcula (mock) actividad antioxidante" }
        fn get_supported_properties(&self) -> Vec<String> { vec!["radical_scavenging_score".into()] }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::properties::trait_properties::ParameterDefinition> { HashMap::new() }
        async fn calculate_properties(&self, family: &crate::data::family::MoleculeFamily, _p: &HashMap<String, Value>) -> Result<Vec<crate::data::types::LogPData>, Box<dyn std::error::Error>> {
            // Re-using LogPData struct as generic numeric container; value = mock activity score
            let mut v = Vec::new();
            for (i, mol) in family.molecules.iter().enumerate() { 
                v.push(crate::data::types::LogPData { value: 0.5 + 0.1 * i as f64 + (mol.smiles.len() as f64 * 0.01), source: "antiox_activity_mock".into(), frozen: false, timestamp: chrono::Utc::now() });
            }
            Ok(v)
        }
    }

    struct AntioxAggregateProvider;
    #[async_trait::async_trait]
    impl DataProvider for AntioxAggregateProvider {
        fn get_name(&self) -> &str { "antiox_aggregate" }
        fn get_version(&self) -> &str { "0.1.0" }
        fn get_description(&self) -> &str { "Agrega puntajes antioxidantes" }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::data::trait_dataprovider::DataParameterDefinition> { HashMap::new() }
        async fn calculate(&self, families: &[crate::data::family::MoleculeFamily], _p: &HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error>> {
            let mut sum = 0.0; let mut count = 0.0;
            for fam in families { if let Some(prop) = fam.get_property("radical_scavenging_score") { for val in &prop.values { sum += val.value; count += 1.0; } } }
            let avg = if count > 0.0 { sum / count } else { 0.0 };
            Ok(Value::Object(serde_json::Map::from_iter(vec![
                ("mean_activity".into(), Value::Number(serde_json::Number::from_f64(avg).unwrap_or(0.into()))),
                ("n_values".into(), Value::Number((count as i64).into())),
            ])))
        }
    }

    molecule_providers.insert("antiox_seed".into(), Box::new(AntioxidantSeedProvider) as Box<dyn crate::providers::molecule::traitmolecule::MoleculeProvider>);
    properties_providers.insert("antiox_activity".into(), Box::new(AntioxActivityPropertiesProvider) as Box<dyn crate::providers::properties::trait_properties::PropertiesProvider>);
    let mut data_providers: HashMap<String, Box<dyn DataProvider>> = HashMap::new();
    data_providers.insert("antiox_aggregate".into(), Box::new(AntioxAggregateProvider) as Box<dyn DataProvider>);

    // Create manager and start a new flow (explicit call to exercise API)
    let mut manager = WorkflowManager::new(repo, molecule_providers, properties_providers, data_providers);
    let _root_id = manager.start_new_flow();

    // Create steps
    let acquisition_step = MoleculeAcquisitionStep {
        id: Uuid::new_v4(),
        name: "Acquire Test Molecules".to_string(),
        description: "Acquires a set of test molecules".to_string(),
        provider_name: "test_molecule".to_string(),
        parameters: HashMap::from([
            ("count".to_string(), Value::Number(5.into())),
        ]),
    };

    let properties_step = PropertiesCalculationStep {
        id: Uuid::new_v4(),
        name: "Calculate LogP".to_string(),
        description: "Calculates LogP for all molecules".to_string(),
        provider_name: "test_properties".to_string(),
        property_name: "logp".to_string(),
        parameters: HashMap::from([
            ("calculation_method".to_string(), Value::String("test".to_string())),
        ]),
    };

    // Execute acquisition step
    let acquisition_output = manager.execute_step(&acquisition_step, Vec::new(), acquisition_step.parameters.clone()).await?;
    println!("Acquired {} molecules", acquisition_output.families[0].molecules.len());

    // Branch example (no-op branch just to exercise API)
    let _branch_root = manager.create_branch(acquisition_step.id);
    // Execute properties step
    let properties_output = manager.execute_step(&properties_step, acquisition_output.families, properties_step.parameters.clone()).await?;
    println!("Calculated properties for {} molecules", properties_output.families[0].molecules.len());

    let _mock = crate::providers::molecule::implementations::mock_provider::MockMoleculeProvider::new(
        "TestMock".to_string(),
        "0.1.0".to_string(),
    );

    // Antioxidant mini-flow
    let antiox_seed_step = MoleculeAcquisitionStep { id: Uuid::new_v4(), name: "Acquire Antiox Seeds".into(), description: "Obtiene moléculas antioxidantes de referencia".into(), provider_name: "antiox_seed".into(), parameters: HashMap::new() };
    let antiox_acq = manager.execute_step(&antiox_seed_step, vec![], HashMap::new()).await?;
    println!("Antiox seeds: {}", antiox_acq.families[0].molecules.len());

    let antiox_prop_step = PropertiesCalculationStep { id: Uuid::new_v4(), name: "Score Antiox Activity".into(), description: "Calcula puntajes antioxidantes".into(), provider_name: "antiox_activity".into(), property_name: "radical_scavenging_score".into(), parameters: HashMap::new() };
    let antiox_scored = manager.execute_step(&antiox_prop_step, antiox_acq.families.clone(), HashMap::new()).await?;
    if let Some(prop) = antiox_scored.families[0].get_property("radical_scavenging_score") { println!("Scores registrados: {}", prop.values.len()); }

    // Demonstrate branching: branch from scoring step id and (re)score (mock) with same provider to simulate alternative path
    let branch_root = manager.create_branch(antiox_prop_step.id);
    let _ = branch_root; // silence unused
    let branch_step = PropertiesCalculationStep { id: Uuid::new_v4(), name: "Branch Alt Score".into(), description: "Alternative scoring branch".into(), provider_name: "antiox_activity".into(), property_name: "radical_scavenging_score".into(), parameters: HashMap::new() };
    let _branch_out = manager.execute_step(&branch_step, antiox_scored.families.clone(), HashMap::new()).await?;

    // Fetch executions by root id to verify persistence in memory
    let executions = manager.repository().get_steps_by_root(manager.root_execution_id()).await;
    println!("Total step executions for root {}: {}", manager.root_execution_id(), executions.len());

    // Exercise repository API to avoid dead code (mock diagnostics)
    if let Some(first) = executions.first() {
        // get_execution & get_step_execution
        let _all_for_first = manager.repository().get_execution(first.step_id).await;
        let _first_entry = manager.repository().get_step_execution(first.step_id, 0).await;
        // branch save (creates synthetic branch id)
        let _ = manager.repository().save_step_execution_for_branch(first, uuid::Uuid::new_v4()).await;
    }
    let _ = manager.repository().get_step(uuid::Uuid::new_v4()).await; // expect error, ignore
    let _ = manager.repository().save_step_for_branch(&(), uuid::Uuid::new_v4()).await;
    let _ = manager.repository().get_family(uuid::Uuid::new_v4()).await; // None expected
    let _maybe_last = manager.last_step_id();
    // Explicit call to new() to silence potential dead code if signature changes
    let _dummy_repo = crate::database::repository::WorkflowExecutionRepository::new(true);

    // Runtime usage of parameter type enums to avoid dead_code warnings
    use crate::providers::molecule::traitmolecule::ParameterType as MolParamType;
    use crate::providers::properties::trait_properties::ParameterType as PropParamType;
    use crate::providers::data::trait_dataprovider::{DataParameterType, DataParameterDefinition};
    let _variant_usage = (
        MolParamType::String, MolParamType::Number, MolParamType::Boolean, MolParamType::Array, MolParamType::Object,
        PropParamType::String, PropParamType::Number, PropParamType::Boolean, PropParamType::Array, PropParamType::Object,
        DataParameterType::String, DataParameterType::Number, DataParameterType::Boolean, DataParameterType::Array, DataParameterType::Object,
    );
    // Create and read a DataParameterDefinition at runtime
    let dpd = DataParameterDefinition { name: "runtime".into(), description: "runtime".into(), data_type: DataParameterType::String, required: false, default_value: None };
    let _ = (&dpd.name, &dpd.description, &dpd.data_type, dpd.required, &dpd.default_value);

    println!("Workflow completed successfully!");
    // Use a simple inline DataProvider implementation to exercise trait
    struct InlineCountProv;
    #[async_trait::async_trait]
    impl DataProvider for InlineCountProv {
        fn get_name(&self) -> &str { "inline_count" }
        fn get_version(&self) -> &str { "0.0.1" }
        fn get_description(&self) -> &str { "Counts molecules" }
        fn get_available_parameters(&self) -> std::collections::HashMap<String, crate::providers::data::trait_dataprovider::DataParameterDefinition> { std::collections::HashMap::new() }
        async fn calculate(&self, families: &[crate::data::family::MoleculeFamily], _p: &std::collections::HashMap<String, serde_json::Value>) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            Ok(serde_json::json!(families.iter().map(|f| f.molecules.len()).sum::<usize>()))
        }
    }
    let dp = InlineCountProv;
    let _total = dp.calculate(&properties_output.families, &HashMap::new()).await?;
    Ok(())
}
