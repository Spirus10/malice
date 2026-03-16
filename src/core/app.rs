//! Central server context shared by command handlers, HTTP routes, and the UI.

use std::{
    io::Result as IoResult,
    path::Path,
    sync::{Arc, RwLock},
};

use uuid::Uuid;

use super::{
    activity::{ActivityEvent, ActivityLog, ActivitySeverity},
    admission::{AdmissionPolicy, PacketRequestContext, RegisterHeaderPolicy},
    implants::{ImplantRecord, ImplantRegistry},
    integrations::{
        ImplantIntegrationRegistry, InstalledPluginSummary, PluginInspection, PluginStore,
        TaskDefinition, UiActionDefinition, DEFAULT_RESULT_ACTION_ID,
    },
    payloads::PayloadRepository,
    router::PacketRouter,
    tasks::{FetchTaskResponse, TaskRecord, TaskResultPayload, TaskService},
};

pub struct ServerContext {
    implants: ImplantRegistry,
    tasks: TaskService,
    payloads: PayloadRepository,
    integrations: RwLock<ImplantIntegrationRegistry>,
    plugins: PluginStore,
    router: PacketRouter,
    activity: ActivityLog,
    admission: Arc<dyn AdmissionPolicy>,
}

impl ServerContext {
    /// Creates a reference-counted server context and wires cyclic router access.
    ///
    /// @return Shared server context containing the registries, router, and task services.
    pub fn new() -> Arc<Self> {
        let plugins = PluginStore::new("plugins");
        let integrations = ImplantIntegrationRegistry::load(&plugins);
        Arc::new_cyclic(|weak| Self {
            implants: ImplantRegistry::new(),
            tasks: TaskService::new(),
            payloads: PayloadRepository::new(),
            integrations: RwLock::new(integrations),
            plugins,
            router: PacketRouter::new(weak.clone()),
            activity: ActivityLog::new(256),
            admission: Arc::new(RegisterHeaderPolicy),
        })
    }

    pub fn router(&self) -> &PacketRouter {
        &self.router
    }

    pub fn implants(&self) -> &ImplantRegistry {
        &self.implants
    }

    pub fn admission(&self) -> &dyn AdmissionPolicy {
        self.admission.as_ref()
    }

    pub fn packet_request_context(
        &self,
        registration_header: Option<String>,
    ) -> PacketRequestContext {
        PacketRequestContext {
            registration_header,
        }
    }

    /// Records one activity event for later display in the UI.
    ///
    /// @param severity Severity level to record.
    /// @param message Human-readable activity description.
    /// @param clientid Optional implant identifier associated with the event.
    /// @param task_id Optional task identifier associated with the event.
    /// @return None.
    pub async fn record_activity(
        &self,
        severity: ActivitySeverity,
        message: impl Into<String>,
        clientid: Option<Uuid>,
        task_id: Option<Uuid>,
    ) {
        self.activity
            .push(severity, message, clientid, task_id)
            .await;
    }

    /// Returns the most recent activity events across all implants.
    ///
    /// @param limit Maximum number of events to return.
    /// @return Activity events ordered from newest to oldest.
    pub async fn recent_activity(&self, limit: usize) -> Vec<ActivityEvent> {
        self.activity.recent(limit).await
    }

    /// Returns the most recent activity events for a specific implant.
    ///
    /// @param clientid Implant identifier used to filter events.
    /// @param limit Maximum number of events to return.
    /// @return Activity events for the selected implant ordered from newest to oldest.
    pub async fn recent_activity_for_implant(
        &self,
        clientid: &Uuid,
        limit: usize,
    ) -> Vec<ActivityEvent> {
        self.activity.recent_for_implant(clientid, limit).await
    }

    pub async fn list_implants(&self) -> Vec<ImplantRecord> {
        self.implants.list().await
    }

    pub async fn implant_info(&self, clientid: &Uuid) -> Option<ImplantRecord> {
        self.implants.get(clientid).await
    }

    /// Queues a named task after validating the target implant and arguments.
    ///
    /// @param clientid Implant identifier that should receive the task.
    /// @param task_kind User-facing task name such as `whoami`.
    /// @param args Additional task arguments supplied by the operator.
    /// @return Queued task record on success, or an I/O error if validation fails.
    pub async fn queue_named_task(
        &self,
        clientid: Uuid,
        task_kind: &str,
        args: &[String],
    ) -> IoResult<TaskRecord> {
        let implant =
            self.implants.get(&clientid).await.ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Unknown implant")
            })?;
        let integration = self
            .integrations
            .read()
            .expect("integration registry lock poisoned")
            .by_implant_type(&implant.identity.implant_type)?;
        let spec = integration
            .build_task(&implant, task_kind, args, &self.payloads)?
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Unknown task kind '{task_kind}' for implant family {}",
                        implant.identity.family.key()
                    ),
                )
            })?;

        let task = self
            .tasks
            .queue_task_for_implant(&implant, integration.id(), spec)
            .await?;
        self.record_activity(
            ActivitySeverity::Info,
            format!("queued {} for {}", task.task_kind, task.clientid),
            Some(task.clientid),
            Some(task.task_id),
        )
        .await;
        Ok(task)
    }

    /// Returns the persisted record for one task, if it exists.
    ///
    /// @param task_id Task identifier to look up.
    /// @return Task record when found, otherwise `None`.
    pub async fn task_result(&self, task_id: &Uuid) -> Option<TaskRecord> {
        self.tasks.get(task_id).await
    }

    /// Returns the most recent tasks across all implants.
    ///
    /// @param limit Maximum number of task records to return.
    /// @return Task records ordered from newest to oldest.
    pub async fn recent_tasks(&self, limit: usize) -> Vec<TaskRecord> {
        self.tasks.recent(limit).await
    }

    /// Returns the most recent tasks for a specific implant.
    ///
    /// @param clientid Implant identifier used to filter task records.
    /// @param limit Maximum number of task records to return.
    /// @return Task records for the selected implant ordered from newest to oldest.
    pub async fn recent_tasks_for_implant(&self, clientid: &Uuid, limit: usize) -> Vec<TaskRecord> {
        self.tasks.recent_for_implant(clientid, limit).await
    }

    pub async fn register_implant(
        &self,
        requested_clientid: Option<Uuid>,
        payload: super::implants::RegisterPayload,
    ) -> IoResult<ImplantRecord> {
        let integration = self
            .integrations
            .read()
            .expect("integration registry lock poisoned")
            .by_implant_type(&payload.implant_type)?;
        integration.validate_registration(&payload)?;
        self.implants
            .register(
                requested_clientid,
                payload,
                integration.family(),
                integration.capabilities().to_vec(),
            )
            .await
    }

    pub async fn fetch_tasks_for_implant(
        &self,
        clientid: Uuid,
        want: usize,
    ) -> IoResult<FetchTaskResponse> {
        let tasks = self.tasks.lease_tasks(clientid, want).await;
        let mut envelopes = Vec::with_capacity(tasks.len());
        for task in tasks {
            let integration = self
                .integrations
                .read()
                .expect("integration registry lock poisoned")
                .by_id(&task.integration_id)?;
            envelopes.push(integration.serialize_task(&task)?);
        }

        Ok(FetchTaskResponse { tasks: envelopes })
    }

    pub async fn record_task_result(&self, payload: TaskResultPayload) -> IoResult<TaskRecord> {
        let task =
            self.tasks.get(&payload.task_id).await.ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Unknown task id")
            })?;
        let integration = self
            .integrations
            .read()
            .expect("integration registry lock poisoned")
            .by_id(&task.integration_id)?;
        let (task_id, status, result) = integration.decode_result(&task, payload)?;
        self.tasks.complete_result(task_id, status, result).await
    }

    pub fn tasking_metadata(
        &self,
        implant: &ImplantRecord,
    ) -> IoResult<super::implants::TaskingMetadata> {
        let integration = self
            .integrations
            .read()
            .expect("integration registry lock poisoned")
            .by_implant_type(&implant.identity.implant_type)?;
        let definitions: &[TaskDefinition] = integration.task_definitions();
        Ok(super::implants::TaskingMetadata {
            command_names: definitions
                .iter()
                .map(|definition| definition.kind.to_string())
                .collect(),
            command_help: definitions
                .iter()
                .map(|definition| definition.usage.to_string())
                .collect(),
        })
    }

    pub fn ui_actions_for_implant(
        &self,
        implant: &ImplantRecord,
    ) -> IoResult<Vec<UiActionDefinition>> {
        let integration = self
            .integrations
            .read()
            .expect("integration registry lock poisoned")
            .by_implant_type(&implant.identity.implant_type)?;
        let mut actions = integration.ui_actions().to_vec();
        actions.push(UiActionDefinition {
            id: DEFAULT_RESULT_ACTION_ID.to_string(),
            label: "view latest result".to_string(),
            task_kind: None,
            args_template: Vec::new(),
            command_template: None,
            queue_immediately: false,
        });
        Ok(actions)
    }

    pub fn list_plugins(&self) -> IoResult<Vec<InstalledPluginSummary>> {
        self.plugins.list_installed()
    }

    pub fn inspect_plugin(
        &self,
        plugin_id: &str,
        version: Option<&str>,
    ) -> IoResult<PluginInspection> {
        self.plugins.inspect(plugin_id, version)
    }

    pub fn install_plugin(&self, source: &Path) -> IoResult<InstalledPluginSummary> {
        self.plugins.install_from_dir(source)
    }

    pub fn activate_plugin(
        &self,
        plugin_id: &str,
        version: Option<&str>,
    ) -> IoResult<InstalledPluginSummary> {
        let summary = self.plugins.activate(plugin_id, version)?;
        self.reload_integrations()?;
        Ok(summary)
    }

    pub fn deactivate_plugin(&self, plugin_id: &str) -> IoResult<()> {
        self.plugins.deactivate(plugin_id)?;
        self.reload_integrations()
    }

    pub fn remove_plugin(&self, plugin_id: &str, version: &str) -> IoResult<()> {
        self.plugins.remove(plugin_id, version)
    }

    pub fn plugin_load_errors(&self) -> Vec<String> {
        self.integrations
            .read()
            .expect("integration registry lock poisoned")
            .load_errors()
            .to_vec()
    }

    fn reload_integrations(&self) -> IoResult<()> {
        let updated = ImplantIntegrationRegistry::load(&self.plugins);
        let mut lock = self
            .integrations
            .write()
            .expect("integration registry lock poisoned");
        *lock = updated;
        Ok(())
    }
}
