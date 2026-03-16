use std::io::{Error, ErrorKind, Result};

use uuid::Uuid;

use crate::util::implants::ImplantRecord;

use super::{repository::TaskRepository, types::TaskRecord, TaskSpec};

#[derive(Clone)]
pub struct TaskService {
    repository: TaskRepository,
}

impl TaskService {
    /// Creates an in-memory task service backed by the default repository.
    ///
    /// @return Task service ready to queue, lease, and complete tasks.
    pub fn new() -> Self {
        Self {
            repository: TaskRepository::new(),
        }
    }

    /// Queues a task for one implant after validating required capabilities.
    ///
    /// @param implant Implant record targeted by the task.
    /// @param spec Concrete task specification to persist.
    /// @return Queued task record or an I/O error if validation fails.
    pub async fn queue_task_for_implant(
        &self,
        implant: &ImplantRecord,
        integration_id: &str,
        spec: TaskSpec,
    ) -> Result<TaskRecord> {
        let required = spec.required_capability();
        if !implant.supports(required) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Implant {} does not support task kind {}",
                    implant.identity.clientid,
                    spec.task_type()
                ),
            ));
        }

        Ok(self
            .repository
            .insert_queued(
                implant.identity.clientid,
                integration_id.to_string(),
                spec,
            )
            .await)
    }

    pub async fn lease_tasks(
        &self,
        clientid: Uuid,
        want: usize,
    ) -> Vec<TaskRecord> {
        self.repository.lease_tasks(clientid, want).await
    }

    pub async fn complete_result(
        &self,
        task_id: Uuid,
        status: super::types::TaskStatus,
        result: super::types::TaskResultData,
    ) -> Result<TaskRecord> {
        self.repository
            .complete(task_id, status, result)
            .await
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "Unknown task id"))
    }

    pub async fn get(&self, task_id: &Uuid) -> Option<TaskRecord> {
        self.repository.get(task_id).await
    }

    pub async fn recent(&self, limit: usize) -> Vec<TaskRecord> {
        self.repository.list_recent(limit).await
    }

    pub async fn recent_for_implant(&self, clientid: &Uuid, limit: usize) -> Vec<TaskRecord> {
        self.repository
            .list_recent_for_implant(clientid, limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::util::implants::{
        ImplantCapability, ImplantFamily, ImplantRecord, ImplantRegistry, RegisterPayload,
    };

    use super::*;

    async fn implant_with_capabilities(capabilities: Vec<ImplantCapability>) -> ImplantRecord {
        let execute_coff = ImplantCapability::from_key("execute_coff");
        let family = if capabilities.contains(&execute_coff) {
            "coff_loader"
        } else {
            "unknown"
        };

        let registry = ImplantRegistry::new();
        let record = registry
            .register(
                None,
                RegisterPayload {
                    implant_type: family.to_string(),
                    protocol_version: 1,
                    hostname: "host".to_string(),
                    username: "user".to_string(),
                    pid: 1,
                    process_name: "proc".to_string(),
                    os: "windows".to_string(),
                    arch: "x64".to_string(),
                },
                if capabilities.contains(&execute_coff) {
                    ImplantFamily::CoffLoader
                } else {
                    ImplantFamily::Unknown("unknown".to_string())
                },
                capabilities.clone(),
            )
            .await
            .unwrap();

        if capabilities == record.capabilities {
            record
        } else {
            ImplantRecord {
                capabilities,
                ..record
            }
        }
    }

    #[tokio::test]
    async fn queue_task_validates_implant_capabilities() {
        let service = TaskService::new();
        let implant = implant_with_capabilities(Vec::new()).await;

        let result = service
            .queue_task_for_implant(
                &implant,
                "test",
                TaskSpec::execute_coff(
                    "whoami.obj".to_string(),
                    vec![0x41],
                    "main".to_string(),
                    Vec::new(),
                ),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn queue_and_complete_execute_coff_task() {
        let service = TaskService::new();
        let implant = implant_with_capabilities(vec![ImplantCapability::from_key("execute_coff")]).await;

        let task = service
            .queue_task_for_implant(
                &implant,
                "test",
                TaskSpec::execute_coff(
                    "whoami.obj".to_string(),
                    vec![0x41],
                    "main".to_string(),
                    Vec::new(),
                ),
            )
            .await
            .unwrap();

        let fetched = service.lease_tasks(implant.identity.clientid, 1).await;
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].task_id, task.task_id);

        let completed = service
            .complete_result(
                task.task_id,
                super::super::types::TaskStatus::Completed,
                super::super::types::TaskResultData::Text {
                    encoding: "utf8".to_string(),
                    data: "host\\user".to_string(),
                },
            )
            .await
            .unwrap();

        match completed.result {
            Some(super::super::types::TaskResultData::Text { data, .. }) => {
                assert_eq!(data, "host\\user")
            }
            _ => panic!("missing text result"),
        }
    }
}
