use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
    sync::Arc,
    time::SystemTime,
};

use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    types::{
        HeartbeatPayload, ImplantIdentity, ImplantRecord, ImplantRuntimeState,
        ImplantStaticMetadata, RegisterPayload,
    },
    ImplantCapability, ImplantFamily,
};

#[derive(Clone)]
pub struct ImplantRegistry {
    implants: Arc<Mutex<HashMap<Uuid, ImplantRecord>>>,
}

impl ImplantRegistry {
    /// Creates an empty implant registry.
    ///
    /// @return Registry ready to store implant records by client ID.
    pub fn new() -> Self {
        Self {
            implants: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Inserts or refreshes the registration record for an implant.
    ///
    /// @param requested_clientid Optional implant identifier previously issued by the server.
    /// @param payload Registration payload describing the host and process.
    /// @return The updated implant record after registration is applied.
    pub async fn register(
        &self,
        requested_clientid: Option<Uuid>,
        payload: RegisterPayload,
        family: ImplantFamily,
        capabilities: Vec<ImplantCapability>,
    ) -> Result<ImplantRecord> {
        let now = SystemTime::now();
        let mut lock = self.implants.lock().await;
        let clientid = match requested_clientid {
            Some(clientid) => {
                if !lock.contains_key(&clientid) {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        "Unknown implant identifier; registration requires a server-issued client ID",
                    ));
                }
                clientid
            }
            None => Uuid::new_v4(),
        };

        let record = lock
            .entry(clientid)
            .and_modify(|record| {
                record.identity.family = family.clone();
                record.identity.implant_type = payload.implant_type.clone();
                record.identity.protocol_version = payload.protocol_version;
                record.static_metadata.hostname = payload.hostname.clone();
                record.static_metadata.username = payload.username.clone();
                record.static_metadata.pid = payload.pid;
                record.static_metadata.process_name = payload.process_name.clone();
                record.static_metadata.os = payload.os.clone();
                record.static_metadata.arch = payload.arch.clone();
                record.capabilities = capabilities.clone();
                record.runtime_state.last_seen = now;
                record.runtime_state.last_heartbeat = now;
                record.runtime_state.current_status = "registered".to_string();
                record.runtime_state.last_error = None;
            })
            .or_insert_with(|| ImplantRecord {
                identity: ImplantIdentity {
                    clientid,
                    family,
                    implant_type: payload.implant_type,
                    protocol_version: payload.protocol_version,
                },
                static_metadata: ImplantStaticMetadata {
                    hostname: payload.hostname,
                    username: payload.username,
                    pid: payload.pid,
                    process_name: payload.process_name,
                    os: payload.os,
                    arch: payload.arch,
                },
                capabilities,
                runtime_state: ImplantRuntimeState {
                    first_seen: now,
                    last_seen: now,
                    last_heartbeat: now,
                    current_status: "registered".to_string(),
                    active_task_id: None,
                    last_error: None,
                },
            });

        Ok(record.clone())
    }

    /// Applies a heartbeat update to an existing implant record.
    ///
    /// @param clientid Implant identifier supplied by the runtime.
    /// @param payload Heartbeat payload containing the current runtime status.
    /// @return Updated implant record or an I/O error if the implant is unknown.
    pub async fn update_heartbeat(
        &self,
        clientid: Uuid,
        payload: HeartbeatPayload,
    ) -> Result<ImplantRecord> {
        let now = SystemTime::now();
        let mut lock = self.implants.lock().await;
        let record = lock
            .get_mut(&clientid)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "Unknown implant"))?;

        record.runtime_state.last_seen = now;
        record.runtime_state.last_heartbeat = now;
        record.runtime_state.current_status = payload.status;
        Ok(record.clone())
    }

    /// Returns all known implant records in stable display order.
    ///
    /// @return Implant records sorted for operator display.
    pub async fn list(&self) -> Vec<ImplantRecord> {
        let lock = self.implants.lock().await;
        let mut records: Vec<_> = lock.values().cloned().collect();
        records.sort_by(|left, right| {
            left.static_metadata
                .hostname
                .cmp(&right.static_metadata.hostname)
                .then(left.identity.clientid.cmp(&right.identity.clientid))
        });
        records
    }

    /// Returns a single implant record by client ID.
    ///
    /// @param clientid Implant identifier to look up.
    /// @return Implant record when found, otherwise `None`.
    pub async fn get(&self, clientid: &Uuid) -> Option<ImplantRecord> {
        self.implants.lock().await.get(clientid).cloned()
    }

    /// Updates the currently active task for an implant, if the record exists.
    ///
    /// @param clientid Implant identifier to update.
    /// @param task_id Active task identifier, or `None` to clear it.
    /// @return None.
    pub async fn set_active_task(&self, clientid: Uuid, task_id: Option<Uuid>) {
        if let Some(record) = self.implants.lock().await.get_mut(&clientid) {
            record.runtime_state.active_task_id = task_id;
        }
    }
}
