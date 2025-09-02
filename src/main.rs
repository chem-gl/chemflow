//! Punto de entrada de ChemFlow.
//! Responsable de:
//! 1. Cargar configuración (.env)
//! 2. Ejecutar migraciones
//! 3. Probar conexión a BD
//! 4. Registrar proveedores de ejemplo
//! 5. Ejecutar un flujo de demostración con branching y agregaciones
mod config;
mod data;
mod database;
mod migrations;
mod molecule;
mod providers;
mod workflow;

use crate::database::repository::WorkflowExecutionRepository;
use crate::providers::data::trait_dataprovider::DataProvider;
use crate::providers::molecule::implementations::test_provider::TestMoleculeProvider;
use crate::providers::properties::implementations::test_provider::TestPropertiesProvider;
// Explicitly bring antioxidant providers so their public structs are
// constructed and not warned as dead code
use crate::providers::data::antioxidant_aggregate_provider::AntioxidantAggregateProvider as ExtAntioxidantAggregateProvider;
use crate::providers::molecule::implementations::antioxidant_seed_provider::AntioxidantSeedProvider as ExtAntioxidantSeedProvider;
use crate::providers::properties::implementations::antioxidant_activity_provider::AntioxidantActivityPropertiesProvider as ExtAntioxidantActivityPropertiesProvider;
use crate::workflow::manager::WorkflowManager;
use crate::workflow::step::{DataAggregationStep, MoleculeAcquisitionStep, PropertiesCalculationStep, StepOutput, MultiMoleculeAcquisitionStep, MultiPropertiesStep, FilterStep};
use crate::providers::properties::implementations::generic_physchem::GenericPhysChemProvider;
use config::{create_pool, CONFIG};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // (1) Cargar .env si existe
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: could not load .env file ({e}) - relying on existing environment");
    }
    println!("Hello, ChemFlow! Running migrations...");

    // (2) Migraciones
    if let Err(e) = migrations::run_migrations().await {
        eprintln!("Failed to run migrations: {e}");
        return Err(e);
    }
    println!("Migrations applied.");

    // (3) Probar conexión BD
    println!("Intentando conectar a la base de datos...");
    println!("URL: {}", CONFIG.database.url);
    let pool = create_pool().await?;
    let row: (i64,) = sqlx::query_as("SELECT $1").bind(1_i64).fetch_one(&pool).await?;
    println!("Conexión verificada, resultado test: {}", row.0);

    let repo = WorkflowExecutionRepository::with_pool(pool).await;

    // (4) Registrar proveedores base
    let mut molecule_providers = HashMap::new();
    molecule_providers.insert("test_molecule".to_string(), Box::new(TestMoleculeProvider::new()) as Box<dyn crate::providers::molecule::traitmolecule::MoleculeProvider>);
    // Construct external antioxidant seed provider (module) to avoid dead_code
    // warning
    let _ext_seed = ExtAntioxidantSeedProvider;

    let mut properties_providers = HashMap::new();
    properties_providers.insert("test_properties".to_string(), Box::new(TestPropertiesProvider::new()) as Box<dyn crate::providers::properties::trait_properties::PropertiesProvider>);
    // Construct external antioxidant activity provider
    let _ext_activity = ExtAntioxidantActivityPropertiesProvider;

    // Mock antioxidant-specific providers (molecule + properties + data)
    struct AntioxidantSeedProvider;
    #[async_trait::async_trait]
    impl crate::providers::molecule::traitmolecule::MoleculeProvider for AntioxidantSeedProvider {
        fn get_name(&self) -> &str {
            "antiox_seed"
        }
        fn get_version(&self) -> &str {
            "0.1.0"
        }
        fn get_description(&self) -> &str {
            "Genera moléculas semilla antioxidantes mock"
        }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::molecule::traitmolecule::ParameterDefinition> {
            HashMap::new()
        }
        async fn get_molecule_family(&self, _p: &HashMap<String, Value>) -> Result<crate::data::family::MoleculeFamily, Box<dyn std::error::Error>> {
            let mut fam = crate::data::family::MoleculeFamily::new("Antioxidant Seeds".into(), Some("Mocked reference antioxidants".into()));
            // Simple canonical seed set (mock SMILES)
            for smi in ["O=CC1=CC=CC(O)=C1O", "CC1=C(O)C=C(O)C=C1O", "C1=CC(=CC=C1O)O"] {
                // e.g. cinnamaldehyde, catechol-like
                let m = crate::molecule::Molecule::from_smiles(smi.to_string())?;
                fam.molecules.push(m);
            }
            Ok(fam)
        }
    }

    struct AntioxActivityPropertiesProvider;
    #[async_trait::async_trait]
    impl crate::providers::properties::trait_properties::PropertiesProvider for AntioxActivityPropertiesProvider {
        fn get_name(&self) -> &str {
            "antiox_activity"
        }
        fn get_version(&self) -> &str {
            "0.1.0"
        }
        fn get_description(&self) -> &str {
            "Calcula (mock) actividad antioxidante"
        }
        fn get_supported_properties(&self) -> Vec<String> {
            vec!["radical_scavenging_score".into()]
        }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::properties::trait_properties::ParameterDefinition> {
            HashMap::new()
        }
        async fn calculate_properties(&self, family: &crate::data::family::MoleculeFamily, _p: &HashMap<String, Value>) -> Result<Vec<crate::data::types::LogPData>, Box<dyn std::error::Error>> {
            // Re-using LogPData struct as generic numeric container; value = mock activity
            // score
            let mut v = Vec::new();
            for (i, mol) in family.molecules.iter().enumerate() {
                v.push(crate::data::types::LogPData { value: 0.5 + 0.1 * i as f64 + (mol.smiles.len() as f64 * 0.01),
                                                      source: "antiox_activity_mock".into(),
                                                      frozen: false,
                                                      timestamp: chrono::Utc::now() });
            }
            Ok(v)
        }
    }

    struct AntioxAggregateProvider;
    #[async_trait::async_trait]
    impl DataProvider for AntioxAggregateProvider {
        fn get_name(&self) -> &str {
            "antiox_aggregate"
        }
        fn get_version(&self) -> &str {
            "0.1.0"
        }
        fn get_description(&self) -> &str {
            "Agrega puntajes antioxidantes"
        }
        fn get_available_parameters(&self) -> HashMap<String, crate::providers::data::trait_dataprovider::DataParameterDefinition> {
            HashMap::new()
        }
        async fn calculate(&self, families: &[crate::data::family::MoleculeFamily], _p: &HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error>> {
            let mut sum = 0.0;
            let mut count = 0.0;
            for fam in families {
                if let Some(prop) = fam.get_property("radical_scavenging_score") {
                    for val in &prop.values {
                        sum += val.value;
                        count += 1.0;
                    }
                }
            }
            let avg = if count > 0.0 { sum / count } else { 0.0 };
            Ok(Value::Object(serde_json::Map::from_iter(vec![("mean_activity".into(), Value::Number(serde_json::Number::from_f64(avg).unwrap_or(0.into()))),
                                                             ("n_values".into(), Value::Number((count as i64).into())),])))
        }
    }

    molecule_providers.insert("antiox_seed".into(), Box::new(AntioxidantSeedProvider) as Box<dyn crate::providers::molecule::traitmolecule::MoleculeProvider>);
    properties_providers.insert("antiox_activity".into(), Box::new(AntioxActivityPropertiesProvider) as Box<dyn crate::providers::properties::trait_properties::PropertiesProvider>);
    let mut data_providers: HashMap<String, Box<dyn DataProvider>> = HashMap::new();
    data_providers.insert("antiox_aggregate".into(), Box::new(AntioxAggregateProvider) as Box<dyn DataProvider>);
    // Construct external antioxidant aggregate provider
    let _ext_agg = ExtAntioxidantAggregateProvider;

    // (5) Crear manager e iniciar nuevo flujo
    let mut manager = WorkflowManager::new(repo, molecule_providers, properties_providers, data_providers);
    let _root_id = manager.start_new_flow();

    // (6) Definir steps de ejemplo
    let acquisition_step = MoleculeAcquisitionStep { id: Uuid::new_v4(),
                                                     name: "Acquire Test Molecules".to_string(),
                                                     description: "Acquires a set of test molecules".to_string(),
                                                     provider_name: "test_molecule".to_string(),
                                                     parameters: HashMap::from([("count".to_string(), Value::Number(5.into()))]) };

    let properties_step = PropertiesCalculationStep { id: Uuid::new_v4(),
                                                      name: "Calculate LogP".to_string(),
                                                      description: "Calculates LogP for all molecules".to_string(),
                                                      provider_name: "test_properties".to_string(),
                                                      property_name: "logp".to_string(),
                                                      parameters: HashMap::from([("calculation_method".to_string(), Value::String("test".to_string()))]) };

    // (7) Ejecutar step de adquisición
    let acquisition_output = manager.execute_step(&acquisition_step, Vec::new(), acquisition_step.parameters.clone()).await?;
    println!("Acquired {} molecules", acquisition_output.families[0].molecules.len());
    // (7b) Congelar explícitamente la familia adquirida para ejercitar
    // freeze_family y asegurar hash
    if let Some(fam) = acquisition_output.families.first() {
        manager.repository().freeze_family(fam.id).await?;
        println!("Family {} frozen (hash now {:?})", fam.id, fam.family_hash);
    }

    // (8) Crear rama (branch) desde el step de adquisición
    let _branch_root = manager.create_branch(acquisition_step.id);
    // (9) Ejecutar step de cálculo de propiedades
    let properties_output = manager.execute_step(&properties_step, acquisition_output.families, properties_step.parameters.clone()).await?;
    println!("Calculated properties for {} molecules", properties_output.families[0].molecules.len());

    let _mock = crate::providers::molecule::implementations::mock_provider::MockMoleculeProvider::new("TestMock".to_string(), "0.1.0".to_string());

    // (10) Mini flujo antioxidante
    let antiox_seed_step = MoleculeAcquisitionStep { id: Uuid::new_v4(),
                                                     name: "Acquire Antiox Seeds".into(),
                                                     description: "Obtiene moléculas antioxidantes de referencia".into(),
                                                     provider_name: "antiox_seed".into(),
                                                     parameters: HashMap::new() };
    let antiox_acq = manager.execute_step(&antiox_seed_step, vec![], HashMap::new()).await?;
    println!("Antiox seeds: {}", antiox_acq.families[0].molecules.len());

    let antiox_prop_step = PropertiesCalculationStep { id: Uuid::new_v4(),
                                                       name: "Score Antiox Activity".into(),
                                                       description: "Calcula puntajes antioxidantes".into(),
                                                       provider_name: "antiox_activity".into(),
                                                       property_name: "radical_scavenging_score".into(),
                                                       parameters: HashMap::new() };
    let antiox_scored = manager.execute_step(&antiox_prop_step, antiox_acq.families.clone(), HashMap::new()).await?;
    if let Some(prop) = antiox_scored.families[0].get_property("radical_scavenging_score") {
        println!("Scores registrados: {}", prop.values.len());
    }

    // (11) Branching desde step de scoring
    let branch_root = manager.create_branch(antiox_prop_step.id);
    let _ = branch_root; // silence unused
    let branch_step = PropertiesCalculationStep { id: Uuid::new_v4(),
                                                  name: "Branch Alt Score".into(),
                                                  description: "Alternative scoring branch".into(),
                                                  provider_name: "antiox_activity".into(),
                                                  property_name: "radical_scavenging_score".into(),
                                                  parameters: HashMap::new() };
    let _branch_out = manager.execute_step(&branch_step, antiox_scored.families.clone(), HashMap::new()).await?;

    // (12) Obtener ejecuciones por root para validar trazabilidad
    let executions = manager.repository().get_steps_by_root(manager.root_execution_id()).await;
    println!("Total step executions for root {}: {}", manager.root_execution_id(), executions.len());

    // (13) Ejercitar API del repositorio para diagnóstico / evitar código muerto
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

    // (14) Uso en runtime de enums de parámetros (evitar dead_code)
    use crate::providers::data::trait_dataprovider::{DataParameterDefinition, DataParameterType};
    use crate::providers::molecule::traitmolecule::ParameterType as MolParamType;
    use crate::providers::properties::trait_properties::ParameterType as PropParamType;
    let _variant_usage = (MolParamType::String,
                          MolParamType::Number,
                          MolParamType::Boolean,
                          MolParamType::Array,
                          MolParamType::Object,
                          PropParamType::String,
                          PropParamType::Number,
                          PropParamType::Boolean,
                          PropParamType::Array,
                          PropParamType::Object,
                          DataParameterType::String,
                          DataParameterType::Number,
                          DataParameterType::Boolean,
                          DataParameterType::Array,
                          DataParameterType::Object);
    // Create and read a DataParameterDefinition at runtime
    let dpd = DataParameterDefinition { name: "runtime".into(),
                                        description: "runtime".into(),
                                        data_type: DataParameterType::String,
                                        required: false,
                                        default_value: None };
    let _ = (&dpd.name, &dpd.description, &dpd.data_type, dpd.required, &dpd.default_value);

    println!("Workflow completed successfully!");
    // Use a simple inline DataProvider implementation to exercise trait
    struct InlineCountProv;
    #[async_trait::async_trait]
    impl DataProvider for InlineCountProv {
        fn get_name(&self) -> &str {
            "inline_count"
        }
        fn get_version(&self) -> &str {
            "0.0.1"
        }
        fn get_description(&self) -> &str {
            "Counts molecules"
        }
        fn get_available_parameters(&self) -> std::collections::HashMap<String, crate::providers::data::trait_dataprovider::DataParameterDefinition> {
            std::collections::HashMap::new()
        }
        async fn calculate(&self, families: &[crate::data::family::MoleculeFamily], _p: &std::collections::HashMap<String, serde_json::Value>) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            Ok(serde_json::json!(families.iter().map(|f| f.molecules.len()).sum::<usize>()))
        }
    }
    let dp = InlineCountProv;
    let _total = dp.calculate(&properties_output.families, &HashMap::new()).await?;

    println!("\n=== Ejecución step-by-step con referencias y branching manual + auto ===");
    manager.start_new_flow();
    let mut recorded_step_ids: Vec<Uuid> = Vec::new();
    // Step A: Acquire test molecules
    let step_a = MoleculeAcquisitionStep { id: Uuid::new_v4(),
                                           name: "A:Acquire".into(),
                                           description: "Acquire base set".into(),
                                           provider_name: "test_molecule".into(),
                                           parameters: HashMap::from([("count".into(), Value::Number(3.into()))]) };
    let out_a = manager.execute_step(&step_a, vec![], step_a.parameters.clone()).await?;
    recorded_step_ids.push(out_a.execution_info.step_id);
    println!("Step A complete -> step_id={} families={}", out_a.execution_info.step_id, out_a.families.len());

    // Step B: Calculate LogP (first variant)
    let step_b = PropertiesCalculationStep { id: Uuid::new_v4(),
                                             name: "B:LogP#1".into(),
                                             description: "LogP baseline".into(),
                                             provider_name: "test_properties".into(),
                                             property_name: "logp".into(),
                                             parameters: HashMap::from([("calculation_method".into(), Value::String("baseline".into()))]) };
    let out_b = manager.execute_step(&step_b, out_a.families.clone(), step_b.parameters.clone()).await?;
    recorded_step_ids.push(out_b.execution_info.step_id);
    println!("Step B complete -> step_id={} prop_count={} branch_from={:?}",
             out_b.execution_info.step_id,
             out_b.families[0].get_property("logp").map(|p| p.values.len()).unwrap_or(0),
             out_b.execution_info.branch_from_step_id);

    // Step C: Calculate LogP (second variant) - triggers auto-branch due to changed
    // parameter hash
    let mut params_c = HashMap::new();
    params_c.insert("calculation_method".into(), Value::String("alt_model".into()));
    let step_c = PropertiesCalculationStep { id: Uuid::new_v4(),
                                             name: "C:LogP#2".into(),
                                             description: "LogP alternative".into(),
                                             provider_name: "test_properties".into(),
                                             property_name: "logp".into(),
                                             parameters: params_c.clone() };
    let out_c = manager.execute_step(&step_c, out_b.families.clone(), params_c).await?;
    recorded_step_ids.push(out_c.execution_info.step_id);
    println!("Step C complete -> auto-branch? branch_from={:?}", out_c.execution_info.branch_from_step_id);

    // Manual branch from step B (using recorded reference) and then run another
    // property calc with freeze
    let branch_origin_id = recorded_step_ids[1]; // step B
    manager.create_branch(branch_origin_id);
    let mut params_d = HashMap::new();
    params_d.insert("calculation_method".into(), Value::String("branch_variant".into()));
    params_d.insert("freeze".into(), Value::Bool(true)); // congelar familia tras este step
    let step_d = PropertiesCalculationStep { id: Uuid::new_v4(),
                                             name: "D:LogP#Branch".into(),
                                             description: "Branch logP variant (frozen)".into(),
                                             provider_name: "test_properties".into(),
                                             property_name: "logp".into(),
                                             parameters: params_d.clone() };
    let out_d = manager.execute_step(&step_d, out_b.families.clone(), params_d).await?;
    recorded_step_ids.push(out_d.execution_info.step_id);
    println!("Step D (branch) complete -> branch_from={:?} frozen={} hash={:?}",
             out_d.execution_info.branch_from_step_id, out_d.families[0].frozen, out_d.families[0].family_hash);

    // Aggregation over the latest (branched) family using DataAggregationStep to
    // showcase multi-family stats (single here for demo)
    let agg_step = DataAggregationStep { id: Uuid::new_v4(),
                                         name: "E:AggregateLogP".into(),
                                         description: "Aggregate LogP stats".into(),
                                         provider_name: "antiox_aggregate".into(),
                                         result_key: "logp_stats".into(),
                                         parameters: HashMap::new() };
    // Use both main-line (out_c) and branch (out_d) families for aggregation
    let mut agg_input_fams = out_c.families.clone();
    agg_input_fams.extend(out_d.families.clone());
    let agg_out = manager.execute_step(&agg_step, agg_input_fams, HashMap::from([("data_provider".into(), Value::String("antiox_aggregate".into()))])).await?;
    println!("Aggregation step result keys: {:?}", agg_out.results.keys().collect::<Vec<_>>());

    // ---------------------------------------------------------------------
    // (16) Ejemplo de ejecución 'full run' (pipeline) en modo lote
    // ---------------------------------------------------------------------
    println!("\n=== Ejecución de pipeline completa (modo batch) ===");
    manager.start_new_flow();
    let p_acq = MoleculeAcquisitionStep { id: Uuid::new_v4(),
                                          name: "BatchAcquire".into(),
                                          description: "Batch acquire".into(),
                                          provider_name: "test_molecule".into(),
                                          parameters: HashMap::from([("count".into(), Value::Number(4.into()))]) };
    let p_props = PropertiesCalculationStep { id: Uuid::new_v4(),
                                              name: "BatchLogP".into(),
                                              description: "Batch logp".into(),
                                              provider_name: "test_properties".into(),
                                              property_name: "logp".into(),
                                              parameters: HashMap::from([("calculation_method".into(), Value::String("batch".into()))]) };
    let p_agg = DataAggregationStep { id: Uuid::new_v4(),
                                      name: "BatchAggregate".into(),
                                      description: "Batch aggregate".into(),
                                      provider_name: "antiox_aggregate".into(),
                                      result_key: "batch_stats".into(),
                                      parameters: HashMap::new() };

    // Simple helper inline to run sequentially
    async fn run_pipeline(manager: &mut WorkflowManager, steps: Vec<(&dyn crate::workflow::step::WorkflowStep, HashMap<String, Value>)>) -> Result<Vec<StepOutput>, Box<dyn std::error::Error>> {
        let mut outputs = Vec::new();
        let mut families: Vec<crate::data::family::MoleculeFamily> = Vec::new();
        for (s, params) in steps {
            let out = manager.execute_step(s, families, params.clone()).await?;
            families = out.families.clone();
            outputs.push(out);
        }
        Ok(outputs)
    }
    let pipeline_results = run_pipeline(&mut manager,
                                        vec![(&p_acq as &dyn crate::workflow::step::WorkflowStep, p_acq.parameters.clone()),
                                             (&p_props as &dyn crate::workflow::step::WorkflowStep, p_props.parameters.clone()),
                                             (&p_agg as &dyn crate::workflow::step::WorkflowStep, HashMap::from([("data_provider".into(), Value::String("antiox_aggregate".into()))])),]).await?;
    println!("Pipeline ejecutada en {} steps. Último tipo outputs={} results_keys={:?}",
             pipeline_results.len(),
             pipeline_results.last().unwrap().families.len(),
             pipeline_results.last().unwrap().results.keys().collect::<Vec<_>>());

    println!("Ejemplos avanzados completados.");
    // (17) DSL estilo encadenado solicitado (step1 -> step2 -> step3, re-ejecutar
    // step2 provoca branch)
    println!("\n=== DSL simplificada (FlowSession) ===");
    use crate::workflow::flowdsl::FlowSession;
    let mut session = FlowSession::new(&mut manager);
    let s1 = session.step1_acquire(4).await?;
    println!("DSL step1 id={s1}");
    let s2 = session.step2_logp("baseline").await?;
    println!("DSL step2 baseline id={s2}");
    // Re-ejecución con diferentes parámetros -> branch
    let s2b = session.step2_logp("alt").await?;
    println!("DSL step2 alt (branch) id={s2b}");
    let s3 = session.step3_aggregate().await?;
    println!("DSL step3 aggregate id={s3}");
    println!("Familias resultantes en DSL: {}", session.current_families().len());

    // ---------------------------------------------------------------
    // (18) Referenciar structs y métodos para evitar dead_code warnings
    // ---------------------------------------------------------------
    let _generic_provider = GenericPhysChemProvider::new();
    if let Some(first_exec) = executions.first() {
        let _ = manager.repository().verify_execution_integrity(first_exec.step_id).await;
    }
    let _ = manager.repository().build_branch_tree(manager.root_execution_id()).await;
    let _ = manager.repository().export_workflow_report(manager.root_execution_id()).await;
    let _ = manager.repository().list_property_values("logp", None).await;
    if let Some(sid) = recorded_step_ids.first().cloned() {
        let _ = manager.reexecute_tail_preview(manager.root_execution_id(), sid).await;
        let _ = manager.reexecute_from(sid, &[]).await;
    }
    // Instantiate step structs without execution (usage marks them as used in non-test build)
    let _multi_acq = MultiMoleculeAcquisitionStep { id: Uuid::new_v4(), name: "DemoMultiAcq".into(), description: "multi".into(), provider_names: vec![], parameters_per_provider: HashMap::new() };
    let _multi_props = MultiPropertiesStep { id: Uuid::new_v4(), name: "DemoMultiProps".into(), description: "multi props".into(), specs: vec![] };
    let _filter = FilterStep { id: Uuid::new_v4(), name: "DemoFilter".into(), description: "filter".into(), property: "logp".into(), min: None, max: None };
    Ok(())
}
