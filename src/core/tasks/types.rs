use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::core::implants::ImplantCapability;

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
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub task_id: Uuid,
    pub status: String,
    pub result_encoding: String,
    pub result_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Leased,
    Acknowledged,
    TimedOut,
    Completed,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Queued => write!(f, "queued"),
            TaskStatus::Leased => write!(f, "leased"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Acknowledged => write!(f, "acknowledged"),
            TaskStatus::TimedOut => write!(f, "timed out"),
            TaskStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub kind: String,
    pub required_capability: ImplantCapability,
    pub state: Value,
}

#[derive(Debug, Clone)]
pub enum TaskResultData {
    Text { encoding: String, data: String },
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: Uuid,
    pub clientid: Uuid,
    pub integration_id: String,
    pub task_kind: String,
    pub state: Value,
    pub queued_at: SystemTime,
    pub leased_at: Option<SystemTime>,
    pub acknowledged_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub status: TaskStatus,
    pub result: Option<TaskResultData>,
}
