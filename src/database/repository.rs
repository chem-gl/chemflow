use std::collections::HashMap;
use uuid::Uuid;
use crate::workflow::step::{StepExecutionInfo, StepStatus};
use crate::data::family::MoleculeFamily;

#[derive(Clone)]
pub struct WorkflowExecutionRepository {
    in_memory: std::sync::Arc<tokio::sync::RwLock<HashMap<Uuid, Vec<StepExecutionInfo>>>>,
    pool: Option<sqlx::Pool<sqlx::Postgres>>,
}

impl WorkflowExecutionRepository {
    pub fn new(_in_memory_only: bool) -> Self {
        Self {
            in_memory: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pool: None, // in-memory only (placeholder for future pool wiring using flag)
        }
    }

    pub async fn with_pool(pool: sqlx::Pool<sqlx::Postgres>) -> Self {
        Self {
            in_memory: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pool: Some(pool),
        }
    }

    pub async fn save_step_execution(&self, execution: &StepExecutionInfo) -> Result<(), Box<dyn std::error::Error>> {
        // Always store in-memory for quick retrieval
        {
            let mut guard = self.in_memory.write().await;
            guard.entry(execution.step_id).or_default().push(execution.clone());
        }
        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO workflow_step_executions (step_id, name, description, status, parameters, providers_used, start_time, end_time)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                 ON CONFLICT (step_id) DO UPDATE SET status = EXCLUDED.status, end_time = EXCLUDED.end_time, parameters = EXCLUDED.parameters, providers_used = EXCLUDED.providers_used"
            )
            .bind(execution.step_id)
            .bind("step") // placeholder name (extend model to include name/description later)
            .bind("")
            .bind(match &execution.status { StepStatus::Pending => "Pending", StepStatus::Running => "Running", StepStatus::Completed => "Completed", StepStatus::Failed(_) => "Failed" })
            .bind(serde_json::to_value(&execution.parameters)?)
            .bind(serde_json::to_value(&execution.providers_used)?)
            .bind(execution.start_time)
            .bind(execution.end_time)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    pub async fn upsert_family(&self, family: &MoleculeFamily) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO molecule_families (id, name, description, molecules, properties, parameters, source_provider)
                 VALUES ($1,$2,$3,$4,$5,$6,$7)
                 ON CONFLICT (id) DO UPDATE SET name=EXCLUDED.name, description=EXCLUDED.description, molecules=EXCLUDED.molecules, properties=EXCLUDED.properties, parameters=EXCLUDED.parameters, source_provider=EXCLUDED.source_provider"
            )
            .bind(family.id)
            .bind(&family.name)
            .bind(&family.description)
            .bind(serde_json::to_value(&family.molecules)?)
            .bind(serde_json::to_value(&family.properties)?)
            .bind(serde_json::to_value(&family.parameters)?)
            .bind(serde_json::to_value(&family.source_provider)?)
            .execute(pool)
            .await?;

            // Insert individual property entries for query
            for (prop_name, entry) in &family.properties {
                for value in &entry.values {
                    sqlx::query(
                        "INSERT INTO molecule_family_properties (family_id, property_name, value, source, frozen, timestamp)
                         VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING"
                    )
                    .bind(family.id)
                    .bind(prop_name)
                    .bind(value.value)
                    .bind(&value.source)
                    .bind(value.frozen)
                    .bind(value.timestamp)
                    .execute(pool)
                    .await?;
                }
            }
        }
        Ok(())
    }

    pub async fn link_step_family(&self, step_id: Uuid, family_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO workflow_step_family (step_id, family_id) VALUES ($1,$2) ON CONFLICT DO NOTHING"
            )
            .bind(step_id)
            .bind(family_id)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_family(&self, id: Uuid) -> Result<Option<MoleculeFamily>, Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            let row = sqlx::query!("SELECT id, name, description, molecules, properties, parameters, source_provider FROM molecule_families WHERE id = $1", id)
                .fetch_optional(pool)
                .await?;
            if let Some(r) = row {
                let molecules_val = r.molecules; // NOT NULL column
                let properties_val = r.properties; // NOT NULL
                let parameters_val = r.parameters; // NOT NULL
                let source_provider_val = r.source_provider.unwrap_or(serde_json::json!(null));
                let family = MoleculeFamily {
                    id: r.id,
                    name: r.name,
                    description: r.description,
                    molecules: serde_json::from_value(molecules_val)?,
                    properties: serde_json::from_value(properties_val)?,
                    parameters: serde_json::from_value(parameters_val)?,
                    source_provider: serde_json::from_value(source_provider_val)?,
                };
                return Ok(Some(family));
            }
        }
        Ok(None)
    }

    pub async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<StepExecutionInfo>, Box<dyn std::error::Error>> {
        // Prefer in-memory
        let guard = self.in_memory.read().await;
        Ok(guard.get(&execution_id).cloned().unwrap_or_default())
    }

    pub async fn get_step_execution(&self, execution_id: Uuid, step_index: usize) -> Result<StepExecutionInfo, Box<dyn std::error::Error>> {
        let all = self.get_execution(execution_id).await?;
        all.get(step_index).cloned().ok_or("Step not found".into())
    }

    pub async fn save_step_execution_for_branch(&self, execution: &StepExecutionInfo, branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let mut cloned = execution.clone();
        cloned.step_id = branch_id; // treat branch as separate id for now
        self.save_step_execution(&cloned).await
    }

    pub async fn get_step(&self, _step_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        Err("Not implemented".into())
    }

    pub async fn save_step_for_branch(&self, _step: &(), _branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Gathers all step executions matching a given root_execution_id (in-memory only for now).
    pub async fn get_steps_by_root(&self, root_id: Uuid) -> Vec<StepExecutionInfo> {
        let guard = self.in_memory.read().await;
        let mut collected = Vec::new();
        for vec_exec in guard.values() {
            for exec in vec_exec {
                if exec.root_execution_id == root_id { collected.push(exec.clone()); }
            }
        }
        collected.sort_by_key(|e| e.start_time);
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::step::StepStatus;
    use std::collections::HashMap;
    use chrono::Utc;

    #[tokio::test]
    async fn test_repository_methods() {
    let repo = WorkflowExecutionRepository::new(true);
        
        let execution_info = StepExecutionInfo {
            step_id: Uuid::new_v4(),
            parameters: HashMap::new(),
            providers_used: Vec::new(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            status: StepStatus::Completed,
            root_execution_id: Uuid::new_v4(),
            parent_step_id: None,
            branch_from_step_id: None,
        };
        
        // Test save_step_execution
        repo.save_step_execution(&execution_info).await.unwrap();
        
        // Test get_execution
        let executions = repo.get_execution(execution_info.step_id).await.unwrap();
        assert_eq!(executions.len(), 1);
        
        // Test get_step_execution
        let step = repo.get_step_execution(execution_info.step_id, 0).await.unwrap();
        assert_eq!(step.step_id, execution_info.step_id);
        
        // Test save_step_execution_for_branch
        let branch_id = Uuid::new_v4();
        repo.save_step_execution_for_branch(&execution_info, branch_id).await.unwrap();
        
        let branch_executions = repo.get_execution(branch_id).await.unwrap();
        assert_eq!(branch_executions.len(), 1);
        
        // Test get_step (will error but calls the method)
        let _ = repo.get_step(Uuid::new_v4()).await;
        
        // Test save_step_for_branch
        repo.save_step_for_branch(&(), Uuid::new_v4()).await.unwrap();

    // Call get_family (will be None in in-memory mode without persisted DB pool)
    let _none = repo.get_family(Uuid::new_v4()).await.unwrap();
        // Test get_steps_by_root (should find entries for existing root ids)
        let list = repo.get_steps_by_root(execution_info.root_execution_id).await;
        assert!(!list.is_empty());
    }
}

#[cfg(test)]
mod repository_usage_tests {
    use super::*;
    use crate::workflow::step::StepStatus;
    use uuid::Uuid;
    use chrono::Utc;

    #[tokio::test]
    async fn test_repo_all_methods() {
    let repo = WorkflowExecutionRepository::new(true);
    let info = StepExecutionInfo { step_id: Uuid::new_v4(), parameters: HashMap::new(), providers_used: Vec::new(), start_time: Utc::now(), end_time: Utc::now(), status: StepStatus::Pending, root_execution_id: Uuid::new_v4(), parent_step_id: None, branch_from_step_id: None };
        repo.save_step_execution(&info).await.unwrap();
        let all = repo.get_execution(info.step_id).await.unwrap();
        assert_eq!(all.len(), 1);
        let one = repo.get_step_execution(info.step_id, 0).await.unwrap();
        assert_eq!(one.step_id, info.step_id);
        let branch = Uuid::new_v4();
    repo.save_step_execution_for_branch(&info, branch).await.unwrap();
        let branched = repo.get_execution(branch).await.unwrap();
        assert_eq!(branched.len(), 1);
        let _ = repo.get_step(Uuid::new_v4()).await;
        repo.save_step_for_branch(&(), Uuid::new_v4()).await.unwrap();
    let _none = repo.get_family(Uuid::new_v4()).await.unwrap();
    let _by_root = repo.get_steps_by_root(info.root_execution_id).await;
    }
}


async fn _use_repository_methods() {
    use crate::workflow::step::{StepExecutionInfo, StepStatus};
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    let repo = WorkflowExecutionRepository::new(true);
    let id = Uuid::new_v4();
    let info = StepExecutionInfo {
        step_id: id,
        parameters: HashMap::new(),
        providers_used: Vec::new(),
        start_time: Utc::now(),
        end_time: Utc::now(),
        status: StepStatus::Completed,
        root_execution_id: Uuid::new_v4(),
        parent_step_id: None,
        branch_from_step_id: None,
    };
    let _ = repo.save_step_execution(&info).await;
    let _ = repo.get_execution(id).await;
    let _ = repo.get_step_execution(id, 0).await;
    let _ = repo.save_step_execution_for_branch(&info, id).await;
    let _ = repo.get_step(id).await;
    let _ = repo.save_step_for_branch(&(), id).await;
}
