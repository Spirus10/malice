use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::SystemTime,
};

use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::{TaskRecord, TaskResultData, TaskSpec, TaskStatus};

#[derive(Clone)]
pub struct TaskRepository {
    tasks: Arc<Mutex<HashMap<Uuid, TaskRecord>>>,
    queues: Arc<Mutex<HashMap<Uuid, VecDeque<Uuid>>>>,
}

impl TaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn insert_queued(&self, clientid: Uuid, spec: TaskSpec) -> TaskRecord {
        let task = TaskRecord {
            task_id: Uuid::new_v4(),
            clientid,
            spec,
            queued_at: SystemTime::now(),
            leased_at: None,
            acknowledged_at: None,
            completed_at: None,
            status: TaskStatus::Queued,
            result: None,
        };

        self.tasks.lock().await.insert(task.task_id, task.clone());
        self.queues
            .lock()
            .await
            .entry(clientid)
            .or_insert_with(VecDeque::new)
            .push_back(task.task_id);

        task
    }

    pub async fn lease_tasks(&self, clientid: Uuid, want: usize) -> Vec<TaskRecord> {
        let mut queues = self.queues.lock().await;
        let Some(queue) = queues.get_mut(&clientid) else {
            return Vec::new();
        };

        let mut task_ids = Vec::new();
        for _ in 0..want.max(1) {
            if let Some(task_id) = queue.pop_front() {
                task_ids.push(task_id);
            } else {
                break;
            }
        }
        drop(queues);

        let now = SystemTime::now();
        let mut tasks = self.tasks.lock().await;
        let mut leased = Vec::new();
        for task_id in task_ids {
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = TaskStatus::Leased;
                task.leased_at = Some(now);
                leased.push(task.clone());
            }
        }

        leased
    }

    pub async fn complete(&self, task_id: Uuid, status: TaskStatus, result: TaskResultData) -> Option<TaskRecord> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks.get_mut(&task_id)?;
        task.status = status;
        task.completed_at = Some(SystemTime::now());
        task.result = Some(result);
        Some(task.clone())
    }

    pub async fn get(&self, task_id: &Uuid) -> Option<TaskRecord> {
        self.tasks.lock().await.get(task_id).cloned()
    }
}
