use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::util::implants::ImplantCapability;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTaskRequest {
    pub want: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTaskResponse {
    pub tasks: Vec<TaskEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub task_id: Uuid,
    pub task_type: String,
    pub object_name: String,
    pub entrypoint: String,
    pub object_encoding: String,
    pub object_data: String,
    pub args_encoding: String,
    pub args_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub task_id: Uuid,
    pub status: String,
    pub result_encoding: String,
    pub result_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TaskStatus {
    Queued,
    Leased,
    Acknowledged,
    TimedOut,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ExecuteCoffTask {
    pub object_name: String,
    pub object_bytes: Vec<u8>,
    pub entrypoint: String,
    pub args: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum TaskSpec {
    ExecuteCoff(ExecuteCoffTask),
}

impl TaskSpec {
    pub fn execute_coff(
        object_name: String,
        object_bytes: Vec<u8>,
        entrypoint: String,
        args: Vec<u8>,
    ) -> Self {
        Self::ExecuteCoff(ExecuteCoffTask {
            object_name,
            object_bytes,
            entrypoint,
            args,
        })
    }

    pub fn task_type(&self) -> &'static str {
        match self {
            Self::ExecuteCoff(_) => "execute_coff",
        }
    }

    pub fn required_capability(&self) -> ImplantCapability {
        match self {
            Self::ExecuteCoff(_) => ImplantCapability::ExecuteCoff,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TaskResultData {
    Text { encoding: String, data: String },
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: Uuid,
    pub clientid: Uuid,
    pub spec: TaskSpec,
    pub queued_at: SystemTime,
    pub leased_at: Option<SystemTime>,
    pub acknowledged_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub status: TaskStatus,
    pub result: Option<TaskResultData>,
}
