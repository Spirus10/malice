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

pub fn print_implant_list(record: &ImplantRecord) {
    println!(
        "{} {}\\{} {} heartbeat={} status={}",
        record.identity.clientid,
        record.static_metadata.hostname,
        record.static_metadata.username,
        record.static_metadata.process_name,
        format_time(record.runtime_state.last_heartbeat),
        record.runtime_state.current_status
    );
}

pub fn print_implant_info(record: &ImplantRecord) {
    println!("clientid: {}", record.identity.clientid);
    println!("implant_type: {}", record.identity.implant_type);
    println!("protocol_version: {}", record.identity.protocol_version);
    println!("hostname: {}", record.static_metadata.hostname);
    println!("username: {}", record.static_metadata.username);
    println!("pid: {}", record.static_metadata.pid);
    println!("process_name: {}", record.static_metadata.process_name);
    println!("os: {}", record.static_metadata.os);
    println!("arch: {}", record.static_metadata.arch);
    println!("first_seen: {}", format_time(record.runtime_state.first_seen));
    println!("last_seen: {}", format_time(record.runtime_state.last_seen));
    println!("last_heartbeat: {}", format_time(record.runtime_state.last_heartbeat));
    println!("status: {}", record.runtime_state.current_status);
    println!("capabilities: {:?}", record.capabilities);
}

pub fn print_task_result(task: &TaskRecord) {
    println!("task_id: {}", task.task_id);
    println!("clientid: {}", task.clientid);
    println!("task_type: {}", task.spec.task_type());
    println!("status: {:?}", task.status);
    println!("queued_at: {}", format_time(task.queued_at));
    if let Some(leased_at) = task.leased_at {
        println!("leased_at: {}", format_time(leased_at));
    }
    if let Some(acknowledged_at) = task.acknowledged_at {
        println!("acknowledged_at: {}", format_time(acknowledged_at));
    }
    if let Some(completed_at) = task.completed_at {
        println!("completed_at: {}", format_time(completed_at));
    }
    if let Some(TaskResultData::Text { encoding, data }) = &task.result {
        println!("result_encoding: {}", encoding);
        println!("result:\n{}", data);
    }
}
