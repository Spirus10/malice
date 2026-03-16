//! Human-readable formatting helpers for command and widget output.

use std::time::SystemTime;

use chrono::{DateTime, Local};

use crate::util::{
    implants::ImplantRecord,
    tasks::{TaskRecord, TaskResultData},
};

pub fn format_time(value: SystemTime) -> String {
    let datetime: DateTime<Local> = value.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_implant_list(record: &ImplantRecord) -> String {
    format!(
        "{} {}\\{} {} heartbeat={} status={}",
        record.identity.clientid,
        record.static_metadata.hostname,
        record.static_metadata.username,
        record.static_metadata.process_name,
        format_time(record.runtime_state.last_heartbeat),
        record.runtime_state.current_status
    )
}

pub fn format_implant_info(record: &ImplantRecord) -> Vec<String> {
    vec![
        format!("clientid: {}", record.identity.clientid),
        format!("implant_type: {}", record.identity.implant_type),
        format!("protocol_version: {}", record.identity.protocol_version),
        format!("hostname: {}", record.static_metadata.hostname),
        format!("username: {}", record.static_metadata.username),
        format!("pid: {}", record.static_metadata.pid),
        format!("process_name: {}", record.static_metadata.process_name),
        format!("os: {}", record.static_metadata.os),
        format!("arch: {}", record.static_metadata.arch),
        format!(
            "first_seen: {}",
            format_time(record.runtime_state.first_seen)
        ),
        format!("last_seen: {}", format_time(record.runtime_state.last_seen)),
        format!(
            "last_heartbeat: {}",
            format_time(record.runtime_state.last_heartbeat)
        ),
        format!("status: {}", record.runtime_state.current_status),
        format!("active_task_id: {:?}", record.runtime_state.active_task_id),
        format!("capabilities: {:?}", record.capabilities),
    ]
}

pub fn format_task_result(task: &TaskRecord) -> Vec<String> {
    let mut lines = vec![
        format!("task_id: {}", task.task_id),
        format!("clientid: {}", task.clientid),
        format!("task_type: {}", task.spec.task_type()),
        format!("status: {:?}", task.status),
        format!("queued_at: {}", format_time(task.queued_at)),
    ];
    if let Some(leased_at) = task.leased_at {
        lines.push(format!("leased_at: {}", format_time(leased_at)));
    }
    if let Some(acknowledged_at) = task.acknowledged_at {
        lines.push(format!("acknowledged_at: {}", format_time(acknowledged_at)));
    }
    if let Some(completed_at) = task.completed_at {
        lines.push(format!("completed_at: {}", format_time(completed_at)));
    }
    if let Some(TaskResultData::Text { encoding, data }) = &task.result {
        lines.push(format!("result_encoding: {}", encoding));
        lines.push("result:".to_string());
        lines.extend(data.lines().map(str::to_string));
    }
    lines
}

pub fn preview_task_result(task: Option<&TaskRecord>) -> Vec<String> {
    let Some(task) = task else {
        return vec!["latest task: none".to_string()];
    };

    let mut lines = vec![
        format!("latest task: {}", task.spec.task_type()),
        format!("task status: {:?}", task.status),
    ];

    match &task.result {
        Some(TaskResultData::Text { data, .. }) => {
            let preview = data.lines().next().unwrap_or_default();
            if preview.is_empty() {
                lines.push("result preview: <empty>".to_string());
            } else {
                lines.push(format!("result preview: {}", preview));
            }
        }
        None => lines.push("result preview: pending".to_string()),
    }

    lines
}
