//! Runtime-backed implementation of `ImplantIntegration`.
//!
//! Translation layers:
//!
//!   package manifest + plugin worker
//!      -> WorkerPluginIntegration
//!      -> validate registration
//!      -> build queued task state
//!      -> serialize leased tasks
//!      -> decode task results
//!
//! This module is the bridge between stable core types (`TaskRecord`,
//! `RegisterPayload`, `TaskEnvelope`) and the plugin RPC protocol defined in
//! `plugin_api.rs`.

use std::{
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use uuid::Uuid;

use crate::core::{
    implants::{ImplantCapability, ImplantFamily, ImplantRecord, RegisterPayload},
    payloads::ArtifactSource,
    tasks::{QueuedTask, TaskEnvelope, TaskRecord, TaskResultData, TaskResultPayload, TaskStatus},
};

use super::{
    manifest::IntegrationManifest,
    plugin_api::{
        registration_payload_value, BuildTaskRequest, BuildTaskResponse, DecodeResultRequest,
        DecodeResultResponse, PluginArtifact, PluginImplantContext, PluginRuntimeHandshake,
        PluginTaskRecord, SerializeTaskRequest, SerializeTaskResponse, OPERATION_BUILD_TASK,
        OPERATION_DECODE_RESULT, OPERATION_GET_MANIFEST, OPERATION_SERIALIZE_TASK,
        OPERATION_VALIDATE_REGISTRATION, SUPPORTED_PLUGIN_API_VERSION,
    },
    types::{ImplantIntegration, TaskDefinition, UiActionDefinition},
    worker::WorkerPluginClient,
};

pub struct WorkerPluginIntegration {
    manifest: IntegrationManifest,
    capabilities: Vec<ImplantCapability>,
    task_definitions: Vec<TaskDefinition>,
    ui_actions: Vec<UiActionDefinition>,
    artifact_roots: Vec<PathBuf>,
    worker: Arc<WorkerPluginClient>,
}

impl WorkerPluginIntegration {
    /// Loads a runtime-backed integration from a validated package manifest.
    ///
    /// Startup sequence:
    ///
    ///   package manifest
    ///      + worker process
    ///      -> runtime handshake (`get_manifest`)
    ///      -> API version check
    ///      -> manifest consistency check
    ///      -> cached capabilities / task defs / UI actions
    pub fn load(
        package_root: &Path,
        manifest: IntegrationManifest,
        worker: WorkerPluginClient,
    ) -> Result<Self> {
        let handshake: PluginRuntimeHandshake = worker.call(OPERATION_GET_MANIFEST, Value::Null)?;
        if handshake.plugin_api_version != SUPPORTED_PLUGIN_API_VERSION {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "Plugin '{}' reported unsupported API version {}",
                    manifest.id, handshake.plugin_api_version
                ),
            ));
        }
        validate_runtime_manifest(&manifest, &handshake.manifest)?;

        let capabilities = manifest.capabilities()?;
        let task_definitions = manifest.task_definitions();
        let ui_actions = manifest.ui_actions();
        let artifact_roots = manifest
            .artifact_roots
            .iter()
            .map(|root| package_root.join(root))
            .collect();

        Ok(Self {
            manifest,
            capabilities,
            task_definitions,
            ui_actions,
            artifact_roots,
            worker: Arc::new(worker),
        })
    }
}

impl ImplantIntegration for WorkerPluginIntegration {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn implant_type(&self) -> &str {
        &self.manifest.implant_type
    }

    fn family(&self) -> ImplantFamily {
        family_from_key(&self.manifest.family)
    }

    fn capabilities(&self) -> &[ImplantCapability] {
        &self.capabilities
    }

    fn task_definitions(&self) -> &[TaskDefinition] {
        &self.task_definitions
    }

    fn ui_actions(&self) -> &[UiActionDefinition] {
        &self.ui_actions
    }

    fn validate_registration(&self, payload: &RegisterPayload) -> Result<()> {
        let result: Value = self.worker.call(
            OPERATION_VALIDATE_REGISTRATION,
            registration_payload_value(payload),
        )?;
        if result.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidInput,
                result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("plugin rejected registration"),
            ))
        }
    }

    fn build_task(
        &self,
        implant: &ImplantRecord,
        task_kind: &str,
        args: &[String],
        artifacts: &dyn ArtifactSource,
    ) -> Result<Option<QueuedTask>> {
        let Some(task) = self
            .manifest
            .tasks
            .iter()
            .find(|task| task.kind == task_kind)
        else {
            return Ok(None);
        };

        let resolved = artifacts.resolve(&task.artifact, &self.artifact_roots)?;
        let request = BuildTaskRequest {
            implant: PluginImplantContext {
                clientid: implant.identity.clientid,
                implant_type: implant.identity.implant_type.clone(),
                family: implant.identity.family.key().to_string(),
                capabilities: implant
                    .capabilities
                    .iter()
                    .map(|capability| capability_key(capability))
                    .collect(),
            },
            task_kind: task_kind.to_string(),
            args: args.to_vec(),
            artifacts: vec![PluginArtifact {
                logical_name: task.artifact.clone(),
                file_name: resolved.file_name,
                bytes_b64: STANDARD.encode(resolved.bytes),
            }],
        };
        let response: BuildTaskResponse = self.worker.call(
            OPERATION_BUILD_TASK,
            serde_json::to_value(request).map_err(invalid_json)?,
        )?;
        Ok(response.queued_task.map(|queued| QueuedTask {
            kind: queued.kind,
            required_capability: ImplantCapability::from_key(&queued.required_capability),
            state: queued.state,
        }))
    }

    fn serialize_task(&self, task: &TaskRecord) -> Result<TaskEnvelope> {
        let request = SerializeTaskRequest {
            task: PluginTaskRecord {
                task_id: task.task_id,
                integration_id: task.integration_id.clone(),
                task_kind: task.task_kind.clone(),
                state: task.state.clone(),
            },
        };
        let response: SerializeTaskResponse = self.worker.call(
            OPERATION_SERIALIZE_TASK,
            serde_json::to_value(request).map_err(invalid_json)?,
        )?;
        Ok(TaskEnvelope {
            task_id: response.task_id,
            task_type: response.task_type,
            fields: response.fields,
        })
    }

    fn decode_result(
        &self,
        task: &TaskRecord,
        payload: TaskResultPayload,
    ) -> Result<(Uuid, TaskStatus, TaskResultData)> {
        let request = DecodeResultRequest {
            task: PluginTaskRecord {
                task_id: task.task_id,
                integration_id: task.integration_id.clone(),
                task_kind: task.task_kind.clone(),
                state: task.state.clone(),
            },
            payload,
        };
        let response: DecodeResultResponse = self.worker.call(
            OPERATION_DECODE_RESULT,
            serde_json::to_value(request).map_err(invalid_json)?,
        )?;
        let status = match response.status.as_str() {
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Plugin returned unsupported task status '{other}'"),
                ))
            }
        };
        match response.result.result_type.as_str() {
            "text" => Ok((
                response.task_id,
                status,
                TaskResultData::Text {
                    encoding: response.result.encoding,
                    data: response.result.data,
                },
            )),
            other => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Plugin returned unsupported result type '{other}'"),
            )),
        }
    }
}

fn validate_runtime_manifest(
    expected: &IntegrationManifest,
    actual: &IntegrationManifest,
) -> Result<()> {
    if expected.id != actual.id
        || expected.implant_type != actual.implant_type
        || expected.plugin_api_version != actual.plugin_api_version
        || expected.protocol_versions != actual.protocol_versions
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "Runtime manifest for plugin '{}' does not match package manifest",
                expected.id
            ),
        ));
    }
    Ok(())
}

/// Maps the manifest family key into the internal implant family enum.
fn family_from_key(value: &str) -> ImplantFamily {
    match value {
        "coff_loader" => ImplantFamily::CoffLoader,
        other => ImplantFamily::Unknown(other.to_string()),
    }
}

/// Normalizes a capability enum back into the plugin string key.
fn capability_key(capability: &ImplantCapability) -> String {
    capability.key().to_string()
}

/// Wraps JSON encoding failures as I/O errors for the integration boundary.
fn invalid_json(err: serde_json::Error) -> Error {
    Error::new(
        ErrorKind::InvalidData,
        format!("Unable to encode plugin payload: {err}"),
    )
}
