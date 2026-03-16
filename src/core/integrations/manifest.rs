use std::{
    fs,
    io::{Error, ErrorKind, Result},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::core::{implants::ImplantCapability, integrations::types::UiActionDefinition};

use super::types::TaskDefinition;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntegrationManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_plugin_api_version")]
    pub plugin_api_version: u32,
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub implant_type: String,
    pub family: String,
    pub protocol_versions: Vec<u32>,
    pub capabilities: Vec<String>,
    pub artifact_roots: Vec<String>,
    pub tasks: Vec<ManifestTaskDefinition>,
    pub ui_actions: Vec<ManifestUiActionDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestTaskDefinition {
    pub kind: String,
    pub usage: String,
    pub artifact: String,
    pub entrypoint: String,
    pub arg_mode: ManifestArgMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ManifestArgMode {
    None,
    Exact { count: usize },
    OptionalSingle { default_value: String },
    ExecuteCommandLine,
    Download,
    UploadFromServer,
    KillPid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestUiActionDefinition {
    pub id: String,
    pub label: String,
    pub task_kind: Option<String>,
    #[serde(default)]
    pub args_template: Vec<String>,
    pub command_template: Option<String>,
    #[serde(default)]
    pub queue_immediately: bool,
}

impl IntegrationManifest {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path).map_err(|err| {
            Error::new(
                err.kind(),
                format!(
                    "Unable to read integration manifest '{}': {err}",
                    path.display()
                ),
            )
        })?;
        serde_json::from_str(&contents).map_err(|err| {
            Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Unable to parse integration manifest '{}': {err}",
                    path.display()
                ),
            )
        })
    }

    pub fn capabilities(&self) -> Result<Vec<ImplantCapability>> {
        Ok(self
            .capabilities
            .iter()
            .map(|capability| ImplantCapability::from_key(capability))
            .collect())
    }

    pub fn task_definitions(&self) -> Vec<TaskDefinition> {
        self.tasks
            .iter()
            .map(|task| TaskDefinition {
                kind: task.kind.clone(),
                usage: task.usage.clone(),
            })
            .collect()
    }

    pub fn ui_actions(&self) -> Vec<UiActionDefinition> {
        self.ui_actions
            .iter()
            .map(|action| UiActionDefinition {
                id: action.id.clone(),
                label: action.label.clone(),
                task_kind: action.task_kind.clone(),
                args_template: action.args_template.clone(),
                command_template: action.command_template.clone(),
                queue_immediately: action.queue_immediately,
            })
            .collect()
    }
}

fn default_schema_version() -> u32 {
    1
}

fn default_plugin_api_version() -> u32 {
    1
}
