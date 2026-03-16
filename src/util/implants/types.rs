use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{capabilities::ImplantCapability, family::ImplantFamily};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    pub implant_type: String,
    pub protocol_version: u32,
    pub hostname: String,
    pub username: String,
    pub pid: u32,
    pub process_name: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub sequence: Option<u64>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ImplantIdentity {
    pub clientid: Uuid,
    pub family: ImplantFamily,
    pub implant_type: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone)]
pub struct ImplantStaticMetadata {
    pub hostname: String,
    pub username: String,
    pub pid: u32,
    pub process_name: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone)]
pub struct ImplantRuntimeState {
    pub first_seen: SystemTime,
    pub last_seen: SystemTime,
    pub last_heartbeat: SystemTime,
    pub current_status: String,
    pub active_task_id: Option<Uuid>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImplantRecord {
    pub identity: ImplantIdentity,
    pub static_metadata: ImplantStaticMetadata,
    pub capabilities: Vec<ImplantCapability>,
    pub runtime_state: ImplantRuntimeState,
}

impl ImplantRecord {
    pub fn supports(&self, capability: ImplantCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskingMetadata {
    pub command_names: Vec<String>,
    pub command_help: Vec<String>,
}
