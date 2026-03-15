use std::io::{Error, ErrorKind, Result};

use uuid::Uuid;

use crate::util::implants::ImplantRecord;

use super::{
    repository::TaskRepository,
    results::decode_result,
    serializer::TaskSerializer,
    types::{FetchTaskResponse, TaskRecord, TaskResultPayload, TaskSpec},
};

#[derive(Clone)]
pub struct TaskService {
    repository: TaskRepository,
    serializer: TaskSerializer,
}

impl TaskService {
    pub fn new() -> Self {
        Self {
            repository: TaskRepository::new(),
            serializer: TaskSerializer,
        }
    }

    pub async fn queue_task_for_implant(
        &self,
        implant: &ImplantRecord,
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
            .insert_queued(implant.identity.clientid, spec)
            .await)
    }

    pub async fn fetch_tasks(&self, clientid: Uuid, want: usize) -> FetchTaskResponse {
        let tasks = self.repository.lease_tasks(clientid, want).await;
        FetchTaskResponse {
            tasks: tasks
                .iter()
                .map(|task| self.serializer.serialize(task))
                .collect(),
        }
    }

    pub async fn record_result(&self, payload: TaskResultPayload) -> Result<TaskRecord> {
        let (task_id, status, result) = decode_result(payload)?;
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
        ImplantCapability, ImplantRecord, ImplantRegistry, RegisterPayload,
    };
    use crate::util::tasks::TaskResultData;

    use super::*;

    async fn implant_with_capabilities(capabilities: Vec<ImplantCapability>) -> ImplantRecord {
        let family = if capabilities.contains(&ImplantCapability::ExecuteCoff) {
            "coff_loader"
        } else {
            "unknown"
        };

        let registry = ImplantRegistry::new();
        let record = registry
            .upsert_registration(
                Uuid::new_v4(),
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
            )
            .await;

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
        let implant = implant_with_capabilities(vec![ImplantCapability::ExecuteCoff]).await;

        let task = service
            .queue_task_for_implant(
                &implant,
                TaskSpec::execute_coff(
                    "whoami.obj".to_string(),
                    vec![0x41],
                    "main".to_string(),
                    Vec::new(),
                ),
            )
            .await
            .unwrap();

        let fetched = service.fetch_tasks(implant.identity.clientid, 1).await;
        assert_eq!(fetched.tasks.len(), 1);
        assert_eq!(fetched.tasks[0].task_id, task.task_id);

        let completed = service
            .record_result(TaskResultPayload {
                task_id: task.task_id,
                status: "success".to_string(),
                result_encoding: "utf8".to_string(),
                result_data: "host\\user".to_string(),
            })
            .await
            .unwrap();

        match completed.result {
            Some(TaskResultData::Text { data, .. }) => assert_eq!(data, "host\\user"),
            _ => panic!("missing text result"),
        }
    }
}
