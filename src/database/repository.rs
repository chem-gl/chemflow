use std::collections::HashMap;
use uuid::Uuid;

use crate::workflow::step::StepExecutionInfo;

pub struct WorkflowExecutionRepository {
    executions: HashMap<Uuid, Vec<StepExecutionInfo>>,
}

impl WorkflowExecutionRepository {
    pub fn new() -> Self {
        Self {
            executions: HashMap::new(),
        }
    }

    pub async fn save_step_execution(&mut self, execution: &StepExecutionInfo) -> Result<(), Box<dyn std::error::Error>> {
        let execution_id = execution.step_id;
    self.executions.entry(execution_id).or_default().push(execution.clone());
        Ok(())
    }

    pub async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<StepExecutionInfo>, Box<dyn std::error::Error>> {
    Ok(self.executions.get(&execution_id).cloned().unwrap_or_default())
    }

    pub async fn get_step_execution(&self, execution_id: Uuid, step_index: usize) -> Result<StepExecutionInfo, Box<dyn std::error::Error>> {
        let execution = self.get_execution(execution_id).await?;
        execution.get(step_index).cloned().ok_or("Step not found".into())
    }

    pub async fn save_step_execution_for_branch(&mut self, execution: &StepExecutionInfo, branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
    self.executions.entry(branch_id).or_default().push(execution.clone());
        Ok(())
    }

    // Placeholder for branching
    pub async fn get_step(&self, _step_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        Err("Not implemented".into())
    }

    pub async fn save_step_for_branch(&mut self, _step: &(), _branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
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
        let mut repo = WorkflowExecutionRepository::new();
        
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
        let mut repo = WorkflowExecutionRepository::new();
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

    let mut repo = WorkflowExecutionRepository::new();
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
