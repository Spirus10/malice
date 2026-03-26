use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::SystemTime,
};

use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::{QueuedTask, TaskRecord, TaskResultData, TaskStatus};

struct TaskRepositoryInner {
    tasks: HashMap<Uuid, TaskRecord>,
    queues: HashMap<Uuid, VecDeque<Uuid>>,
}

#[derive(Clone)]
pub struct TaskRepository {
    inner: Arc<Mutex<TaskRepositoryInner>>,
}

impl TaskRepository {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TaskRepositoryInner {
                tasks: HashMap::new(),
                queues: HashMap::new(),
            })),
        }
    }

    pub async fn insert_queued(
        &self,
        clientid: Uuid,
        integration_id: String,
        task: QueuedTask,
    ) -> TaskRecord {
        let task = TaskRecord {
            task_id: Uuid::new_v4(),
            clientid,
            integration_id,
            task_kind: task.kind,
            state: task.state,
            queued_at: SystemTime::now(),
            leased_at: None,
            acknowledged_at: None,
            completed_at: None,
            status: TaskStatus::Queued,
            result: None,
        };

        let mut inner = self.inner.lock().await;
        inner.tasks.insert(task.task_id, task.clone());
        inner
            .queues
            .entry(clientid)
            .or_insert_with(VecDeque::new)
            .push_back(task.task_id);

        task
    }

    pub async fn lease_tasks(&self, clientid: Uuid, want: usize) -> Vec<TaskRecord> {
        let mut inner = self.inner.lock().await;
        let mut task_ids = Vec::new();

        if let Some(queue) = inner.queues.get_mut(&clientid) {
            for _ in 0..want.max(1) {
                if let Some(task_id) = queue.pop_front() {
                    task_ids.push(task_id);
                } else {
                    break;
                }
            }
        }

        let now = SystemTime::now();
        let mut leased = Vec::new();
        for task_id in task_ids {
            if let Some(task) = inner.tasks.get_mut(&task_id) {
                task.status = TaskStatus::Leased;
                task.leased_at = Some(now);
                leased.push(task.clone());
            }
        }

        leased
    }

    pub async fn complete(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        result: TaskResultData,
    ) -> Option<TaskRecord> {
        let mut inner = self.inner.lock().await;
        let task = inner.tasks.get_mut(&task_id)?;
        task.status = status;
        task.completed_at = Some(SystemTime::now());
        task.result = Some(result);
        Some(task.clone())
    }

    pub async fn get(&self, task_id: &Uuid) -> Option<TaskRecord> {
        self.inner.lock().await.tasks.get(task_id).cloned()
    }

    pub async fn list_recent(&self, limit: usize) -> Vec<TaskRecord> {
        let inner = self.inner.lock().await;
        let mut tasks: Vec<_> = inner.tasks.values().cloned().collect();
        tasks.sort_by(|left, right| {
            right
                .completed_at
                .unwrap_or(right.queued_at)
                .cmp(&left.completed_at.unwrap_or(left.queued_at))
        });
        tasks.truncate(limit);
        tasks
    }

    pub async fn list_recent_for_implant(&self, clientid: &Uuid, limit: usize) -> Vec<TaskRecord> {
        let inner = self.inner.lock().await;
        let mut tasks: Vec<_> = inner
            .tasks
            .values()
            .filter(|task| &task.clientid == clientid)
            .cloned()
            .collect();
        tasks.sort_by(|left, right| {
            right
                .completed_at
                .unwrap_or(right.queued_at)
                .cmp(&left.completed_at.unwrap_or(left.queued_at))
        });
        tasks.truncate(limit);
        tasks
    }
}
