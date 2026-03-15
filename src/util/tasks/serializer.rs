use base64::{engine::general_purpose::STANDARD, Engine as _};

use super::types::{TaskEnvelope, TaskRecord, TaskSpec};

#[derive(Clone, Default)]
pub struct TaskSerializer;

impl TaskSerializer {
    pub fn serialize(&self, task: &TaskRecord) -> TaskEnvelope {
        match &task.spec {
            TaskSpec::ExecuteCoff(spec) => TaskEnvelope {
                task_id: task.task_id,
                task_type: task.spec.task_type().to_string(),
                object_name: spec.object_name.clone(),
                entrypoint: spec.entrypoint.clone(),
                object_encoding: "base64".to_string(),
                object_data: STANDARD.encode(&spec.object_bytes),
                args_encoding: "base64".to_string(),
                args_data: STANDARD.encode(&spec.args),
            },
        }
    }
}
