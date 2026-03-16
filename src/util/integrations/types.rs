use std::io::Result;

use uuid::Uuid;

use crate::util::{
    implants::{ImplantCapability, ImplantFamily, ImplantRecord, RegisterPayload},
    payloads::ArtifactSource,
    tasks::{
        TaskEnvelope, TaskRecord, TaskResultData, TaskResultPayload, TaskSpec, TaskStatus,
    },
};

pub const DEFAULT_RESULT_ACTION_ID: &str = "view_latest_result";

#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub kind: String,
    pub usage: String,
}

#[derive(Debug, Clone)]
pub struct UiActionDefinition {
    pub id: String,
    pub label: String,
    pub task_kind: Option<String>,
    pub args_template: Vec<String>,
    pub command_template: Option<String>,
    pub queue_immediately: bool,
}

pub trait ImplantIntegration: Send + Sync {
    fn id(&self) -> &str;
    fn implant_type(&self) -> &str;
    fn family(&self) -> ImplantFamily;
    fn supported_protocol_versions(&self) -> &[u32];
    fn capabilities(&self) -> &[ImplantCapability];
    fn task_definitions(&self) -> &[TaskDefinition];
    fn ui_actions(&self) -> &[UiActionDefinition];

    fn validate_registration(&self, payload: &RegisterPayload) -> Result<()>;

    fn build_task(
        &self,
        implant: &ImplantRecord,
        task_kind: &str,
        args: &[String],
        artifacts: &dyn ArtifactSource,
    ) -> Result<Option<TaskSpec>>;

    fn serialize_task(&self, task: &TaskRecord) -> Result<TaskEnvelope>;

    fn decode_result(
        &self,
        task: &TaskRecord,
        payload: TaskResultPayload,
    ) -> Result<(Uuid, TaskStatus, TaskResultData)>;
}
