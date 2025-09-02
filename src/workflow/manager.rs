//! Orquestador principal del workflow.
//! Se encarga de:
//! - Mantener el contexto de ejecución raíz (root_execution_id).
//! - Registrar el encadenamiento de steps (parent_step_id) y origen de
//!   bifurcaciones (branch_from_step_id).
//! - Ejecutar steps aplicando inmutabilidad lógica y persistiendo metadatos en
//!   el repositorio.
//! - Persistir familias y su relación con cada ejecución de step para
//!   trazabilidad.
//! - Exponer métodos para iniciar nuevos flujos y crear ramas.
use std::collections::HashMap;

use crate::data::family::MoleculeFamily;
use crate::database::repository::WorkflowExecutionRepository;
use crate::providers::data::trait_dataprovider::DataProvider;
use crate::providers::molecule::traitmolecule::MoleculeProvider;
use crate::providers::properties::trait_properties::PropertiesProvider;
use crate::workflow::step::{StepInput, StepOutput, WorkflowStep};

pub struct WorkflowManager {
    execution_repo: WorkflowExecutionRepository,
    molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
    properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
    data_providers: HashMap<String, Box<dyn DataProvider>>,
    /// Identificador de la raíz del flujo actual (constante a lo largo de la
    /// línea principal y sus ramas derivadas).
    current_root_execution_id: uuid::Uuid,
    /// Último step ejecutado (para encadenar parent_step_id en el siguiente).
    last_step_id: Option<uuid::Uuid>,
    /// Step desde el cual se originó la rama vigente (None si estamos en la
    /// línea principal sin branch activo).
    branch_origin: Option<uuid::Uuid>,
}

impl WorkflowManager {
    pub fn new(execution_repo: WorkflowExecutionRepository,
               molecule_providers: HashMap<String, Box<dyn MoleculeProvider>>,
               properties_providers: HashMap<String, Box<dyn PropertiesProvider>>,
               data_providers: HashMap<String, Box<dyn DataProvider>>)
               -> Self {
        Self { execution_repo,
               molecule_providers,
               properties_providers,
               data_providers,
               current_root_execution_id: uuid::Uuid::new_v4(),
               last_step_id: None,
               branch_origin: None }
    }

    /// Devuelve el identificador raíz del flujo actual.
    pub fn root_execution_id(&self) -> uuid::Uuid {
        self.current_root_execution_id
    }
    /// Devuelve el último step ejecutado (para encadenamiento lineal).
    pub fn last_step_id(&self) -> Option<uuid::Uuid> {
        self.last_step_id
    }
    /// Acceso al repositorio de persistencia (ejecuciones + familias).
    pub fn repository(&self) -> &WorkflowExecutionRepository {
        &self.execution_repo
    }

    /// Inicia un nuevo flujo independiente, generando un nuevo
    /// root_execution_id y reseteando la cadena de parent/branch. Útil
    /// cuando el usuario desea comenzar una ejecución completamente
    /// separada.
    pub fn start_new_flow(&mut self) -> uuid::Uuid {
        self.current_root_execution_id = uuid::Uuid::new_v4();
        self.last_step_id = None;
        self.branch_origin = None;
        self.current_root_execution_id
    }

    /// Crea una rama a partir de un step previo: conserva el root_execution_id
    /// (para agrupar todas las ejecuciones relacionadas) pero marca
    /// branch_origin para que los steps posteriores anoten en su metadata
    /// de ejecución desde qué punto divergen.
    pub fn create_branch(&mut self, from_step_id: uuid::Uuid) -> uuid::Uuid {
        // Keep same root, mark branch origin so subsequent executions record it
        self.last_step_id = Some(from_step_id);
        self.branch_origin = Some(from_step_id);
        self.current_root_execution_id
    }

    pub async fn execute_step(&mut self, step: &dyn WorkflowStep, input_families: Vec<MoleculeFamily>, step_parameters: HashMap<String, serde_json::Value>) -> Result<StepOutput, Box<dyn std::error::Error>> {
        // 0. Auto-branch: si cambia el hash de parámetros O cambia el hash agregado de
        //    familias de entrada -> nueva rama lógica.
        if step.allows_branching() {
            // Obtener última ejecución previa (si existe) una sola vez.
            let prev_exec_opt = if let Some(prev_id) = self.last_step_id {
                match self.execution_repo.get_execution(prev_id).await {
                    Ok(prev_execs) => prev_execs.last().cloned(),
                    Err(_) => None,
                }
            } else {
                None
            };

            let prospective_param_hash = crate::database::repository::compute_sorted_hash(&step_parameters);
            let param_changed = match &prev_exec_opt {
                Some(prev_exec) => prev_exec.parameter_hash.as_deref() != Some(&prospective_param_hash),
                None => true,
            };

            let input_families_hash = crate::database::repository::compute_sorted_hash(&input_families.iter()
                                                                                                      .map(|f| {
                                                                                                          serde_json::json!({
                                                                                                              "id": f.id,
                                                                                                              "family_hash": f.family_hash,
                                                                                                              "properties": f.properties.keys().collect::<Vec<_>>()
                                                                                                          })
                                                                                                      })
                                                                                                      .collect::<Vec<_>>());
            let input_changed = match &prev_exec_opt {
                Some(prev_exec) => prev_exec.parameters.get("_input_families_hash").and_then(|v| v.as_str()) != Some(&input_families_hash),
                None => true,
            };

            if (param_changed || input_changed) && self.last_step_id.is_some() && self.branch_origin != self.last_step_id {
                self.branch_origin = self.last_step_id;
            }
        }
        // 1. Construir StepInput (familias + parámetros específicos de esta
        //    invocación).
        let input = StepInput { families: input_families,
                                parameters: step_parameters };

        // 2. (Previsional) Acceso a metadatos de data_providers: en el futuro se
        //    podrían crear steps de agregación que necesiten estos proveedores; esto
        //    garantiza que la API se use y se detecten cambios tempranamente.
        for (k, prov) in &self.data_providers {
            let _ = k;
            let _ = prov.get_name();
            let _ = prov.get_version();
            let _ = prov.get_description();
            let _ = prov.get_available_parameters();
        }

        let _ = step.get_id();
        let _ = step.get_name();
        let _ = step.get_description();
        let _ = step.get_required_input_types();
        let _ = step.get_output_types();
        let _ = step.allows_branching();
        // 3. Ejecutar el step concreto (obtiene familias/resultados + execution_info
        //    preliminar).
        // Marcamos inicio antes de ejecutar para registrar tiempos correctamente.
        let start_time = chrono::Utc::now();
        let mut output = step.execute(input, &self.molecule_providers, &self.properties_providers, &self.data_providers).await?;
        // Ajustamos tiempos reales.
        output.execution_info.start_time = start_time;
        output.execution_info.end_time = chrono::Utc::now();

        // 4. Enriquecer metadata de ejecución con contexto global de workflow (root,
        //    parent, branch).
        let mut exec = output.execution_info.clone();
        exec.root_execution_id = self.current_root_execution_id;
        exec.parent_step_id = self.last_step_id;
        exec.branch_from_step_id = self.branch_origin;
        exec.input_family_ids = output.families.iter().map(|f| f.id).collect();
        self.last_step_id = Some(exec.step_id);

        // (Legacy source_provider removal) ahora la provenance inicial se asigna
        // mediante creación explícita en steps de adquisición si fuera necesario.

        // Hook: si el step es DataAggregationStep (detectable por output_types).
        // Ejecutar DataProvider real.
        if step.get_output_types().contains(&"aggregation_result".to_string()) && output.execution_info.parameters.get("data_provider").and_then(|v| v.as_str()).and_then(|name| self.data_providers.get(name).map(|_| name)).is_some() {
            let provider_name = output.execution_info.parameters.get("data_provider").and_then(|v| v.as_str()).unwrap();
            if let Some(dp) = self.data_providers.get(provider_name) {
                let families_snapshot = output.families.clone();
                let result_value = dp.calculate(&families_snapshot, &output.execution_info.parameters).await?;
                output.results.insert("aggregation".to_string(), result_value);
            }
        }

        // 5. Persistir la ejecución (in-memory + opcionalmente DB) para reconstrucción
        //    posterior.
        self.execution_repo.save_step_execution(&exec).await?;
        // 6. Devolver execution_info enriquecido al llamador.
        output.execution_info = exec.clone();

        // 7. Persistir familias (upsert) y la relación step->family (histórico y
        //    consultas trazables).
        for fam in &mut output.families {
            // Congelar si se solicitó antes de persistir, para que hash refleje estado
            // congelado.
            if output.execution_info.parameters.get("freeze") == Some(&serde_json::json!(true)) {
                fam.freeze();
            }
            // Recalcular hash (internamente usa inchikeys + parámetros + propiedades +
            // flags).
            fam.recompute_hash();
            if let Err(e) = self.execution_repo.upsert_family(fam).await {
                eprintln!("[persist][family] Error upserting family {}: {e}", fam.id);
            } else {
                eprintln!("[persist][family] Upsert ok family {} molecules={} props={} frozen={} hash={:?}",
                          fam.id,
                          fam.molecules.len(),
                          fam.properties.len(),
                          fam.frozen,
                          fam.family_hash);
            }
            let _ = self.execution_repo.link_step_family(exec.step_id, fam.id).await;
        }

        // Persistir results individuales distinguiendo el tipo
        // (aggregation/property/raw).
        if !output.results.is_empty() {
            let result_type = if step.get_output_types().contains(&"aggregation_result".to_string()) {
                "aggregation"
            } else if step.get_output_types().contains(&"molecule_family".to_string()) && !output.results.is_empty() {
                // PropertiesCalculationStep usualmente no coloca resultados en results (añade a
                // familias), pero si lo hiciera lo marcamos property.
                "property"
            } else {
                "raw"
            };
            let _ = self.execution_repo.upsert_step_results_typed(exec.step_id, &output.results, result_type).await;
        }
        // Snapshot adicional: conteo de moléculas insertadas tras persistencia (solo si
        // provider_type molecule)
        if exec.providers_used.iter().any(|p| p.provider_type == "molecule") && self.execution_repo.pool().is_some() {
            let pool = self.execution_repo.pool().unwrap();
            if let Ok((count_mols,)) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM molecules").fetch_one(pool).await {
                let snap = serde_json::json!({"molecules_total": count_mols, "families_in_step": output.families.len()});
                let mut snap_map = std::collections::HashMap::new();
                snap_map.insert("snapshot_molecule_counts".to_string(), snap);
                let _ = self.execution_repo.upsert_step_results_typed(exec.step_id, &snap_map, "snapshot").await;
            }
        }

        // 8. Resultado final con familias y snapshot de ejecución completamente
        //    contextualizado.
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::molecule::traitmolecule::MoleculeProvider;
    use crate::providers::properties::trait_properties::PropertiesProvider;
    use crate::workflow::step::{StepExecutionInfo, StepInput, StepOutput, StepStatus, WorkflowStep};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use uuid::Uuid;

    struct DummyStep;

    #[async_trait]
    impl WorkflowStep for DummyStep {
        fn get_id(&self) -> Uuid {
            Uuid::new_v4()
        }
        fn get_name(&self) -> &str {
            "dummy"
        }
        fn get_description(&self) -> &str {
            "dummy desc"
        }
        fn get_required_input_types(&self) -> Vec<String> {
            vec![]
        }
        fn get_output_types(&self) -> Vec<String> {
            vec![]
        }
        fn allows_branching(&self) -> bool {
            false
        }

        async fn execute(&self,
                         input: StepInput,
                         _m: &HashMap<String, Box<dyn MoleculeProvider>>,
                         _p: &HashMap<String, Box<dyn PropertiesProvider>>,
                         _d: &HashMap<String, Box<dyn crate::providers::data::trait_dataprovider::DataProvider>>)
                         -> Result<StepOutput, Box<dyn std::error::Error>> {
            Ok(StepOutput { families: input.families.clone(),
                            results: input.parameters.clone(),
                            execution_info: StepExecutionInfo { step_id: Uuid::new_v4(),
                                                                parameters: input.parameters.clone(),
                                                                parameter_hash: Some(crate::database::repository::compute_sorted_hash(&input.parameters)),
                                                                providers_used: Vec::new(),
                                                                start_time: chrono::Utc::now(),
                                                                end_time: chrono::Utc::now(),
                                                                status: StepStatus::Completed,
                                                                root_execution_id: Uuid::new_v4(),
                                                                parent_step_id: None,
                                                                branch_from_step_id: None,
                                                                input_family_ids: Vec::new() } })
        }
    }

    #[tokio::test]
    async fn test_execute_step_manager() {
        let repo = WorkflowExecutionRepository::new(true);
        let mol = HashMap::new();
        let props = HashMap::new();
        let data = HashMap::new();
        let mut manager = WorkflowManager::new(repo, mol, props, data);

        let dummy = DummyStep;
        let families: Vec<MoleculeFamily> = Vec::new();
        let mut params = HashMap::new();
        params.insert("key".to_string(), serde_json::json!("value"));

        let output = manager.execute_step(&dummy, families.clone(), params.clone()).await.expect("manager exec");
        assert_eq!(output.families.len(), families.len());
        assert_eq!(output.results.get("key").unwrap(), &serde_json::json!("value"));
        assert!(matches!(output.execution_info.status, StepStatus::Completed));
    }

    #[tokio::test]
    async fn test_branching_methods_usage() {
        let repo = WorkflowExecutionRepository::new(true);
        let mut manager = WorkflowManager::new(repo, HashMap::new(), HashMap::new(), HashMap::new());
        let original_root = manager.current_root_execution_id;
        let new_root = manager.start_new_flow();
        assert_ne!(original_root, new_root);
        // Create a dummy previous step id and branch from it
        let prev = Uuid::new_v4();
        let same_root = manager.create_branch(prev);
        assert_eq!(same_root, manager.current_root_execution_id);
        assert_eq!(manager.last_step_id, Some(prev));
    }
}
