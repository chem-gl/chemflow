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

    // Create providers
    let mut molecule_providers = HashMap::new();
    molecule_providers.insert("test_molecule".to_string(), Box::new(TestMoleculeProvider::new()) as Box<dyn crate::providers::molecule::traitmolecule::MoleculeProvider>);

    let mut properties_providers = HashMap::new();
    properties_providers.insert("test_properties".to_string(), Box::new(TestPropertiesProvider::new()) as Box<dyn crate::providers::properties::trait_properties::PropertiesProvider>);

    // Create manager
    let mut manager = WorkflowManager::new(repo, molecule_providers, properties_providers);

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

    // Execute properties step
    let properties_output = manager.execute_step(&properties_step, acquisition_output.families, properties_step.parameters.clone()).await?;
    println!("Calculated properties for {} molecules", properties_output.families[0].molecules.len());

    let _mock = crate::providers::molecule::implementations::mock_provider::MockMoleculeProvider::new(
        "TestMock".to_string(),
        "0.1.0".to_string(),
    );

    println!("Workflow completed successfully!");
    Ok(())
}
