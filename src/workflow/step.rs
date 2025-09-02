//! Módulo que define los "steps" (pasos) ejecutables del workflow.
//! Cada step encapsula una operación atómica sobre familias de moléculas
//! (adquisición, cálculo de propiedades, agregaciones futuras, filtrados, etc.)
//! y produce nueva(s) familia(s) y/o resultados auxiliares.
//! 
//! Principios clave implementados aquí:
//! - Inmutabilidad lógica: cada ejecución genera un snapshot (StepExecutionInfo)
//!   con parámetros + proveedores para trazabilidad y reproducibilidad.
//! - Trazabilidad completa: se guarda qué proveedor se usó, su versión y parámetros.
//! - Branching: la metadata del step incluye referencias para reconstruir ramas
//!   (root_execution_id, parent_step_id, branch_from_step_id).
//! - Validación de parámetros: se rellenan valores por defecto y se validan requeridos.
//!
//! Este archivo también provee dos implementaciones concretas: adquisición de moléculas
//! y cálculo de propiedades.
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::data::family::{MoleculeFamily, ProviderReference};
use crate::providers::molecule::traitmolecule::{MoleculeProvider};
use crate::providers::properties::trait_properties::PropertiesProvider;
use crate::providers::data::trait_dataprovider::DataProvider;
use crate::data::types::MolecularData;
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct StepInput {
    /// Familias de moléculas de entrada (puede ser vacío si el step genera nuevas).
    pub families: Vec<MoleculeFamily>,
    /// Parámetros específicos para esta ejecución del step (no mezclados con defaults todavía).
    pub parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepOutput {
    /// Familias de salida resultantes (puede contener las mismas mutadas / extendidas o nuevas).
    pub families: Vec<MoleculeFamily>,
    /// Resultados auxiliares arbitrarios (por ejemplo estadísticas agregadas, valores JSON).
    pub results: HashMap<String, serde_json::Value>,
    /// Información de ejecución enriquecida para trazabilidad y branching.
    pub execution_info: StepExecutionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionInfo {
    /// Identificador único de este step (cada ejecución concreta tiene su propio UUID).
    pub step_id: Uuid,
    /// Parámetros efectivos usados (ya con defaults aplicados y sólo los relevantes para reproducir).
    pub parameters: HashMap<String, serde_json::Value>,
    /// Hash canónico (ordenado) de los parámetros para detectar divergencias y habilitar auto-branch.
    pub parameter_hash: Option<String>,
    /// Lista de proveedores involucrados (puede haber más de uno en steps compuestos futuros).
    pub providers_used: Vec<ProviderReference>,
    /// Marca de tiempo de inicio de la ejecución.
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Marca de tiempo de finalización.
    pub end_time: chrono::DateTime<chrono::Utc>,
    /// Estado final del step (Completed / Failed con mensaje, etc.).
    pub status: StepStatus,
    // Root execution flow id (constant for original workflow run)
    /// Identificador de la raíz del workflow: todas las ramas derivadas comparten este valor.
    pub root_execution_id: Uuid,
    // Parent step id (previous step in linear flow) if any
    /// Referencia al step previo lineal (None si es el primero de la raíz o si se re-ejecuta aislado).
    pub parent_step_id: Option<Uuid>,
    // If this execution is part of a branch, which step it branched from
    /// Si es parte de una rama, indica desde qué step exacto se originó la bifurcación.
    pub branch_from_step_id: Option<Uuid>,
    /// IDs de familias de entrada utilizadas en este step para reconstrucción de dependencias.
    pub input_family_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    /// Creado pero aún no iniciado.
    Pending,
    /// En ejecución.
    Running,
    /// Finalizó correctamente.
    Completed,
    /// Falló con un mensaje de error.
    Failed(String),
}

#[async_trait]
pub trait WorkflowStep: Send + Sync {
    /// Identificador estable del tipo/instancia lógica del step (no de la ejecución).
    fn get_id(&self) -> Uuid;
    /// Nombre amigable para reportes / UI.
    fn get_name(&self) -> &str;
    /// Descripción funcional del propósito del step.
    fn get_description(&self) -> &str;
    /// Tipos de entrada requeridos (ej: ["molecule_family"]). Vacío significa que puede iniciar un workflow.
    fn get_required_input_types(&self) -> Vec<String>;
    /// Tipos de salida producidos.
    fn get_output_types(&self) -> Vec<String>;
    /// Indica si desde este step es lícito bifurcar (crear una rama) al cambiar parámetros.
    fn allows_branching(&self) -> bool;
    
    /// Ejecuta el step con las familias y parámetros dados, retornando nuevas familias y
    /// la metadata de ejecución. Debe ser puro respecto a los datos de entrada (no mutar
    /// estructuras externas) y garantizar que toda la info necesaria para reproducir esté
    /// en `execution_info`.
    async fn execute(
        &self,
        input: StepInput,
        molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
        data_providers: &HashMap<String, Box<dyn DataProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>>;
}

// ---- Parameter validation helpers ----
/// Valida y completa parámetros para proveedores de moléculas.
/// 1. Verifica parámetros requeridos.
/// 2. Aplica valores por defecto donde falten claves opcionales.
/// 3. Devuelve un mapa listo para ser usado en ejecución y trazabilidad.
fn validate_parameters(
    provided: &HashMap<String, Value>,
    definitions: &HashMap<String, crate::providers::molecule::traitmolecule::ParameterDefinition>,
) -> Result<HashMap<String, Value>, String> {
    let mut result = provided.clone();
    for (k, def) in definitions {
        if !result.contains_key(k) {
            if def.required {
                return Err(format!("Missing required parameter: {k}"));
            }
            if let Some(default) = &def.default_value {
                result.insert(k.clone(), default.clone());
            }
        }
    }
    Ok(result)
}

fn validate_prop_parameters(
    provided: &HashMap<String, Value>,
    definitions: &HashMap<String, crate::providers::properties::trait_properties::ParameterDefinition>,
) -> Result<HashMap<String, Value>, String> {
    let mut result = provided.clone();
    for (k, def) in definitions {
        if !result.contains_key(k) {
            if def.required {
                return Err(format!("Missing required parameter: {k}"));
            }
            if let Some(default) = &def.default_value {
                result.insert(k.clone(), default.clone());
            }
        }
    }
    Ok(result)
}

// Implementaciones concretas de steps
/// Step que genera una nueva familia de moléculas a partir de un proveedor de moléculas.
/// No consume familias previas (inicio de flujo o rama). Registra el proveedor y parámetros
/// usados para permitir reproducibilidad y branching posterior.
pub struct MoleculeAcquisitionStep {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub provider_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl WorkflowStep for MoleculeAcquisitionStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_description(&self) -> &str {
        &self.description
    }
    
    fn get_required_input_types(&self) -> Vec<String> {
        Vec::new() // No requiere input
    }
    
    fn get_output_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn allows_branching(&self) -> bool {
        true
    }
    
    async fn execute(
        &self,
        _input: StepInput,
        molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        _properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
        _data_providers: &HashMap<String, Box<dyn DataProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        // 1. Localizar proveedor registrado.
        let provider = molecule_providers.get(&self.provider_name)
            .ok_or_else(|| format!("Provider {} not found", self.provider_name))?;
        // 2. (Opcional) Acceso a metadatos para asegurar uso de API y futuras validaciones.
         let _ = provider.get_name();
        let _ = provider.get_version();
        let _ = provider.get_description();
            let mol_params = provider.get_available_parameters();
            for pd in mol_params.values() {
                let _ = &pd.name;
                let _ = &pd.description;
                let _ = &pd.data_type;
                let _ = &pd.required;
                let _ = &pd.default_value;
            }
  
        let param_defs = provider.get_available_parameters();
        // 3. Validar / completar parámetros.
        let validated = validate_parameters(&self.parameters, &param_defs)
            .map_err(|e| format!("Parameter validation failed: {e}"))?;
        // 4. Ejecutar proveedor para construir familia base (origen de la trazabilidad).
        let mut family = provider.get_molecule_family(&validated).await?;
        // Recompute family hash on creation
        if let Some(h) = crate::database::repository::compute_sorted_hash(&serde_json::json!({
            "molecules": family.molecules.iter().map(|m| &m.inchikey).collect::<Vec<_>>(),
            "properties": family.properties.keys().collect::<Vec<_>>(),
            "parameters": family.parameters,
        })).into() { family.family_hash = Some(h); }
        
        Ok(StepOutput {
            families: vec![family],
            results: HashMap::new(),
            // 5. Construir snapshot de ejecución. (root_execution_id / parent se
            // sobre-escribirán en el Manager al persistir / encadenar.)
            execution_info: StepExecutionInfo {
                step_id: self.id,
                parameters: validated.clone(),
                parameter_hash: Some(crate::database::repository::compute_sorted_hash(&validated)),
                providers_used: vec![ProviderReference {
                    provider_type: "molecule".to_string(),
                    provider_name: self.provider_name.clone(),
                    provider_version: provider.get_version().to_string(),
                    execution_parameters: self.parameters.clone(),
                    execution_id: Uuid::new_v4(),
                }],
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                status: StepStatus::Completed,
                root_execution_id: Uuid::new_v4(),
                parent_step_id: None,
                branch_from_step_id: None,
                input_family_ids: Vec::new(),
            },
        })
    }
}

/// Step que calcula una propiedad específica para cada familia de entrada
/// usando un proveedor de propiedades. Añade (o sobrescribe) la propiedad en cada
/// familia con la información de proveedor y parámetros para trazabilidad.
pub struct PropertiesCalculationStep {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub provider_name: String,
    pub property_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Step que agrega datos (estadísticas, métricas) a partir de múltiples familias usando un DataProvider.
/// No modifica las familias; coloca el resultado JSON en StepOutput.results bajo una clave proporcionada.
pub struct DataAggregationStep {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub provider_name: String,
    pub result_key: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl WorkflowStep for DataAggregationStep {
    fn get_id(&self) -> Uuid { self.id }
    fn get_name(&self) -> &str { &self.name }
    fn get_description(&self) -> &str { &self.description }
    fn get_required_input_types(&self) -> Vec<String> { vec!["molecule_family".to_string()] }
    fn get_output_types(&self) -> Vec<String> { vec!["aggregation_result".to_string()] }
    fn allows_branching(&self) -> bool { true }

    async fn execute(
        &self,
        input: StepInput,
        _molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        _properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
        data_providers: &HashMap<String, Box<dyn DataProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        let provider = data_providers.get(&self.provider_name)
            .ok_or_else(|| format!("Data provider {} not found", self.provider_name))?;
        let result_value = provider.calculate(&input.families, &self.parameters).await?;
        let mut results = HashMap::new();
        results.insert(self.result_key.clone(), result_value);
        Ok(StepOutput {
            families: input.families.clone(),
            results,
            execution_info: StepExecutionInfo {
                step_id: self.id,
                parameters: self.parameters.clone(),
                parameter_hash: Some(crate::database::repository::compute_sorted_hash(&self.parameters)),
                providers_used: vec![], // Podríamos registrar DataProviderReference especializado si se desea
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                status: StepStatus::Completed,
                root_execution_id: Uuid::new_v4(),
                parent_step_id: None,
                branch_from_step_id: None,
                input_family_ids: input.families.iter().map(|f| f.id).collect(),
            },
        })
    }
}

#[async_trait]
impl WorkflowStep for PropertiesCalculationStep {
    fn get_id(&self) -> Uuid {
        self.id
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_description(&self) -> &str {
        &self.description
    }
    
    fn get_required_input_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn get_output_types(&self) -> Vec<String> {
        vec!["molecule_family".to_string()]
    }
    
    fn allows_branching(&self) -> bool {
        true
    }
    
    async fn execute(
        &self,
        input: StepInput,
        _molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
        properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
        _data_providers: &HashMap<String, Box<dyn DataProvider>>,
    ) -> Result<StepOutput, Box<dyn std::error::Error>> {
        // 1. Resolver proveedor de propiedades.
        let provider = properties_providers.get(&self.provider_name)
            .ok_or_else(|| format!("Provider {} not found", self.provider_name))?;
         let _ = provider.get_name();
        let _ = provider.get_version();
        let _ = provider.get_description();
            let _ = provider.get_supported_properties();
            let prop_params = provider.get_available_parameters();
            for pd in prop_params.values() {
                let _ = &pd.name;
                let _ = &pd.description;
                let _ = &pd.data_type;
                let _ = &pd.required;
                let _ = &pd.default_value;
            }
 
        
        let param_defs = provider.get_available_parameters();
        // 2. Validar / normalizar parámetros.
        let validated = validate_prop_parameters(&self.parameters, &param_defs)
            .map_err(|e| format!("Parameter validation failed: {e}"))?;
        let mut output_families = input.families.clone();
        for family in &mut output_families {
            // 3. Calcular datos crudos de la propiedad.
            let properties = provider.calculate_properties(family, &validated).await?;
            for data in &properties {
                let _ = data.get_value();
                let _ = data.get_source();
                let _ = data.is_frozen();
            }
            let _ = family.get_property(&self.property_name);
            // 4. Registrar propiedad + referencia de proveedor (trazabilidad a nivel familia).
            family.add_property(self.property_name.clone(), properties, ProviderReference {
                provider_type: "properties".to_string(),
                provider_name: self.provider_name.clone(),
                provider_version: provider.get_version().to_string(),
                execution_parameters: self.parameters.clone(),
                execution_id: Uuid::new_v4(),
            }, Some(self.id));
        }
        
        Ok(StepOutput {
            families: output_families,
            results: HashMap::new(),
            // 5. Snapshot de ejecución (enriquecido posteriormente por el Manager).
            execution_info: StepExecutionInfo {
                step_id: self.id,
                parameters: validated.clone(),
                parameter_hash: Some(crate::database::repository::compute_sorted_hash(&validated)),
                providers_used: vec![ProviderReference {
                    provider_type: "properties".to_string(),
                    provider_name: self.provider_name.clone(),
                    provider_version: provider.get_version().to_string(),
                    execution_parameters: self.parameters.clone(),
                    execution_id: Uuid::new_v4(),
                }],
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                status: StepStatus::Completed,
                root_execution_id: Uuid::new_v4(),
                parent_step_id: None,
                branch_from_step_id: None,
                input_family_ids: input.families.iter().map(|f| f.id).collect(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::molecule::Molecule;
    use crate::data::family::MoleculeFamily;
    use crate::providers::molecule::implementations::test_provider::TestMoleculeProvider;
    use crate::providers::properties::implementations::test_provider::TestPropertiesProvider;

    struct TestWorkflowStep {
        id: Uuid,
        name: String,
        description: String,
    }

    #[async_trait]
    impl WorkflowStep for TestWorkflowStep {
        fn get_id(&self) -> Uuid {
            self.id
        }
        fn get_name(&self) -> &str {
            &self.name
        }
        fn get_description(&self) -> &str {
            &self.description
        }
        fn get_required_input_types(&self) -> Vec<String> {
            vec!["test_input".to_string()]
        }
        fn get_output_types(&self) -> Vec<String> {
            vec!["test_output".to_string()]
        }
        fn allows_branching(&self) -> bool {
            true
        }
        async fn execute(
            &self,
            _input: StepInput,
            _molecule_providers: &HashMap<String, Box<dyn MoleculeProvider>>,
            _properties_providers: &HashMap<String, Box<dyn PropertiesProvider>>,
            _data_providers: &HashMap<String, Box<dyn DataProvider>>,
        ) -> Result<StepOutput, Box<dyn std::error::Error>> {
            Ok(StepOutput {
                families: Vec::new(),
                results: HashMap::new(),
                execution_info: StepExecutionInfo {
                    step_id: self.id,
                    parameters: HashMap::new(),
                    parameter_hash: Some(crate::database::repository::compute_sorted_hash(&HashMap::<String, serde_json::Value>::new())),
                    providers_used: Vec::new(),
                    start_time: chrono::Utc::now(),
                    end_time: chrono::Utc::now(),
                    status: StepStatus::Completed,
                    root_execution_id: Uuid::new_v4(),
                    parent_step_id: None,
                    branch_from_step_id: None,
                    input_family_ids: Vec::new(),
                },
            })
        }
    }

   
    #[test]
    fn test_workflow_step_methods() {
        let step = TestWorkflowStep {
            id: Uuid::new_v4(),
            name: "Test Step".to_string(),
            description: "Test Description".to_string(),
        };

        assert_eq!(step.get_name(), "Test Step");
        assert_eq!(step.get_description(), "Test Description");
        assert_eq!(step.get_required_input_types(), vec!["test_input".to_string()]);
        assert_eq!(step.get_output_types(), vec!["test_output".to_string()]);
        assert!(step.allows_branching());
    
    }

    #[tokio::test]
    async fn test_molecule_acquisition_step_execute() {
        let mut mol_providers = HashMap::new();
        mol_providers.insert(
            "test_molecule".to_string(),
            Box::new(TestMoleculeProvider::new()) as Box<dyn MoleculeProvider>
        );
        let props_providers: HashMap<String, Box<dyn PropertiesProvider>> = HashMap::new();
        // Create step
        let step = MoleculeAcquisitionStep {
            id: Uuid::new_v4(),
            name: "Acquire".to_string(),
            description: "Acquire molecules".to_string(),
            provider_name: "test_molecule".to_string(),
            parameters: HashMap::new(),
        };
        // Execute
        let input = StepInput { families: Vec::new(), parameters: HashMap::new() };
    let output = step.execute(input, &mol_providers, &props_providers, &HashMap::new())
            .await.expect("execution should succeed");
        // Assertions
        assert_eq!(output.families.len(), 1);
        let family = &output.families[0];
        assert_eq!(family.molecules.len(), 10);
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
        assert_eq!(output.execution_info.providers_used.len(), 1);
        let prov_ref = &output.execution_info.providers_used[0];
        assert_eq!(prov_ref.provider_type, "molecule");
        assert_eq!(prov_ref.provider_name, "test_molecule");
    }

    #[tokio::test]
    async fn test_properties_calculation_step_execute() {
        // Setup provider
        let mol_providers: HashMap<String, Box<dyn MoleculeProvider>> = HashMap::new();
        let mut props_providers = HashMap::new();
        props_providers.insert(
            "test_properties".to_string(),
            Box::new(TestPropertiesProvider::new()) as Box<dyn PropertiesProvider>
        );
        // Prepare input family
        let mut family = MoleculeFamily::new("fam".to_string(), None);
        family.molecules.push(Molecule::new(
            "K".to_string(), "CC".to_string(), "I".to_string(), None
        ));
        let input = StepInput {
            families: vec![family.clone()],
            parameters: HashMap::new(),
        };
        // Create step
        let step = PropertiesCalculationStep {
            id: Uuid::new_v4(),
            name: "Calc".to_string(),
            description: "Calculate properties".to_string(),
            provider_name: "test_properties".to_string(),
            property_name: "logp".to_string(),
            parameters: HashMap::new(),
        };
        // Execute
    let output = step.execute(input, &mol_providers, &props_providers, &HashMap::new())
            .await.expect("execution should succeed");
        // Assertions
        assert_eq!(output.families.len(), 1);
    let out_family = &output.families[0];
    // After execution, property 'logp' should be present
    let prop = out_family.get_property("logp");
    assert!(prop.is_some());
    assert!(!prop.unwrap().values.is_empty());
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
        let prov_ref = &output.execution_info.providers_used[0];
        assert_eq!(prov_ref.provider_type, "properties");
        assert_eq!(prov_ref.provider_name, "test_properties");
    }
}