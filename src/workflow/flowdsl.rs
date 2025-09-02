//! DSL simplificada para ejecutar un flujo con pasos ordenados y branching
//! automático Ejemplo de uso:
//! let mut session = FlowSession::new(&mut manager);
//! let s1 = session.step1_acquire(5).await?;
//! let s2 = session.step2_logp("baseline").await?;
//! // Re-ejecutar step2 con distintos parámetros crea rama
//! let s2_branch = session.step2_logp("alt").await?;
//! let s3 = session.step3_aggregate().await?;
//! // session.step3_aggregate() antes de step2 -> error
use crate::data::family::MoleculeFamily;
use crate::workflow::manager::WorkflowManager;
use crate::workflow::step::{DataAggregationStep, MoleculeAcquisitionStep, PropertiesCalculationStep};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

pub struct FlowSession<'a> {
    manager: &'a mut WorkflowManager,
    families: Vec<MoleculeFamily>,
    executed_order: Vec<&'static str>,
    last_step_ids: HashMap<&'static str, Uuid>,
    last_params_hash: HashMap<&'static str, String>,
}

impl<'a> FlowSession<'a> {
    pub fn new(manager: &'a mut WorkflowManager) -> Self {
        manager.start_new_flow();
        Self { manager,
               families: Vec::new(),
               executed_order: Vec::new(),
               last_step_ids: HashMap::new(),
               last_params_hash: HashMap::new() }
    }

    fn require(&self, required: &[&'static str]) -> Result<(), String> {
        for r in required {
            if !self.executed_order.contains(r) {
                return Err(format!("Paso requerido '{r}' no ejecutado aún"));
            }
        }
        Ok(())
    }

    fn maybe_branch(&mut self, logical: &'static str, params: &HashMap<String, Value>) {
        let hash = crate::database::repository::compute_sorted_hash(params);
        if let Some(prev_step_id) = self.last_params_hash.get(logical).filter(|prev_hash| *prev_hash != &hash).and_then(|_| self.last_step_ids.get(logical)) {
            self.manager.create_branch(*prev_step_id);
        }
        self.last_params_hash.insert(logical, hash);
    }

    pub async fn step1_acquire(&mut self, count: u32) -> Result<Uuid, Box<dyn std::error::Error>> {
        // No prerequisitos
        let params = HashMap::from([("count".into(), Value::Number(count.into()))]);
        self.maybe_branch("step1", &params);
        let step = MoleculeAcquisitionStep { id: Uuid::new_v4(),
                                             name: "Step1:Acquire".into(),
                                             description: "Adquiere moléculas base".into(),
                                             provider_name: "test_molecule".into(),
                                             parameters: params.clone() };
        let out = self.manager.execute_step(&step, vec![], params).await?;
        self.families = out.families.clone();
        self.executed_order.push("step1");
        self.last_step_ids.insert("step1", step.id);
        Ok(step.id)
    }

    pub async fn step2_logp(&mut self, method: &str) -> Result<Uuid, Box<dyn std::error::Error>> {
        self.require(&["step1"])?;
        let params = HashMap::from([("calculation_method".into(), Value::String(method.to_string()))]);
        self.maybe_branch("step2", &params);
        let step = PropertiesCalculationStep { id: Uuid::new_v4(),
                                               name: format!("Step2:LogP({method})"),
                                               description: "Calcula LogP".into(),
                                               provider_name: "test_properties".into(),
                                               property_name: "logp".into(),
                                               parameters: params.clone() };
        let out = self.manager.execute_step(&step, self.families.clone(), params).await?;
        self.families = out.families.clone();
        if !self.executed_order.contains(&"step2") {
            self.executed_order.push("step2");
        }
        self.last_step_ids.insert("step2", step.id);
        Ok(step.id)
    }

    pub async fn step3_aggregate(&mut self) -> Result<Uuid, Box<dyn std::error::Error>> {
        self.require(&["step1", "step2"])?;
        let params: HashMap<String, Value> = HashMap::from([("data_provider".into(), Value::String("antiox_aggregate".into()))]);
        self.maybe_branch("step3", &params);
        let step = DataAggregationStep { id: Uuid::new_v4(),
                                         name: "Step3:Aggregate".into(),
                                         description: "Agrega propiedades".into(),
                                         provider_name: "antiox_aggregate".into(),
                                         result_key: "agg".into(),
                                         parameters: HashMap::new() };
        let _out = self.manager.execute_step(&step, self.families.clone(), params).await?;
        if !self.executed_order.contains(&"step3") {
            self.executed_order.push("step3");
        }
        self.last_step_ids.insert("step3", step.id);
        Ok(step.id)
    }

    pub fn current_families(&self) -> &Vec<MoleculeFamily> {
        &self.families
    }
}
