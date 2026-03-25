//! Wire types for the JSON-RPC-like plugin worker protocol.
//!
//! Message flow:
//!
//!   core -> PluginRequestEnvelope  -> worker stdin
//!   core <- PluginResponseEnvelope <- worker stdout
//!
//! Operation families:
//!
//!   get_manifest
//!   validate_registration
//!   build_task
//!   serialize_task
//!   decode_result
//!
//! Each operation uses the shared request/response envelope plus an
//! operation-specific payload schema defined in this module.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::core::{
    implants::RegisterPayload, integrations::manifest::IntegrationManifest,
    tasks::TaskResultPayload,
};

pub const SUPPORTED_PLUGIN_API_VERSION: u32 = 1;

pub const OPERATION_GET_MANIFEST: &str = "get_manifest";
pub const OPERATION_VALIDATE_REGISTRATION: &str = "validate_registration";
pub const OPERATION_BUILD_TASK: &str = "build_task";
pub const OPERATION_SERIALIZE_TASK: &str = "serialize_task";
pub const OPERATION_DECODE_RESULT: &str = "decode_result";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequestEnvelope {
    pub request_id: String,
    pub operation: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponseEnvelope {
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntimeHandshake {
    pub plugin_api_version: u32,
    pub manifest: IntegrationManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginImplantContext {
    pub clientid: Uuid,
    pub implant_type: String,
    pub family: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginArtifact {
    pub logical_name: String,
    pub file_name: String,
    pub bytes_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTaskRequest {
    pub implant: PluginImplantContext,
    pub task_kind: String,
    pub args: Vec<String>,
    pub artifacts: Vec<PluginArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginQueuedTask {
    pub kind: String,
    pub required_capability: String,
    pub state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTaskResponse {
    pub queued_task: Option<PluginQueuedTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTaskRecord {
    pub task_id: Uuid,
    pub integration_id: String,
    pub task_kind: String,
    pub state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializeTaskRequest {
    pub task: PluginTaskRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializeTaskResponse {
    pub task_id: Uuid,
    pub task_type: String,
    pub fields: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeResultRequest {
    pub task: PluginTaskRecord,
    pub payload: TaskResultPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResultBody {
    #[serde(rename = "type")]
    pub result_type: String,
    pub encoding: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeResultResponse {
    pub task_id: Uuid,
    pub status: String,
    pub result: PluginResultBody,
}

/// Generates a request id used to correlate one plugin request/response pair.
pub fn request_id() -> String {
    Uuid::new_v4().to_string()
}

/// Converts a registration payload into generic JSON for the plugin RPC layer.
pub fn registration_payload_value(payload: &RegisterPayload) -> Value {
    serde_json::to_value(payload).expect("register payload should always serialize")
}
