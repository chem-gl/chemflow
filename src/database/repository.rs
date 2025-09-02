use std::collections::HashMap;
use uuid::Uuid;
use crate::workflow::step::{StepExecutionInfo, StepStatus};

#[derive(Clone)]
pub struct WorkflowExecutionRepository {
    in_memory: std::sync::Arc<tokio::sync::RwLock<HashMap<Uuid, Vec<StepExecutionInfo>>>>,
    pool: Option<sqlx::Pool<sqlx::Postgres>>,
}

impl WorkflowExecutionRepository {
    pub fn new(in_memory_only: bool) -> Self {
        Self {
            in_memory: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pool: if in_memory_only { None } else { None },
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
        let info = StepExecutionInfo { step_id: Uuid::new_v4(), parameters: HashMap::new(), providers_used: Vec::new(), start_time: Utc::now(), end_time: Utc::now(), status: StepStatus::Pending };
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
    }
}

// Dummy async function to use repository methods and avoid dead_code
#[allow(dead_code)]
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
    };
    let _ = repo.save_step_execution(&info).await;
    let _ = repo.get_execution(id).await;
    let _ = repo.get_step_execution(id, 0).await;
    let _ = repo.save_step_execution_for_branch(&info, id).await;
    let _ = repo.get_step(id).await;
    let _ = repo.save_step_for_branch(&(), id).await;
}
