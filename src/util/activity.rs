//! In-memory activity log used by the TUI and HTTP handlers.

use std::{collections::VecDeque, sync::Arc, time::SystemTime};

use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySeverity {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct ActivityEvent {
    pub timestamp: SystemTime,
    pub severity: ActivitySeverity,
    pub message: String,
    pub clientid: Option<Uuid>,
    pub task_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct ActivityLog {
    events: Arc<Mutex<VecDeque<ActivityEvent>>>,
    capacity: usize,
}

impl ActivityLog {
    /// Creates an activity log that retains up to `capacity` events.
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub async fn push(
        &self,
        severity: ActivitySeverity,
        message: impl Into<String>,
        clientid: Option<Uuid>,
        task_id: Option<Uuid>,
    ) {
        let mut events = self.events.lock().await;
        events.push_front(ActivityEvent {
            timestamp: SystemTime::now(),
            severity,
            message: message.into(),
            clientid,
            task_id,
        });

        while events.len() > self.capacity {
            events.pop_back();
        }
    }

    pub async fn recent(&self, limit: usize) -> Vec<ActivityEvent> {
        self.events
            .lock()
            .await
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn recent_for_implant(&self, clientid: &Uuid, limit: usize) -> Vec<ActivityEvent> {
        self.events
            .lock()
            .await
            .iter()
            .filter(|event| event.clientid.as_ref() == Some(clientid))
            .take(limit)
            .cloned()
            .collect()
    }
}
