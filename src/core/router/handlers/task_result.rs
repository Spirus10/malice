use std::{future::Future, pin::Pin, sync::Arc};

use hyper::StatusCode;
use serde_json::json;

use crate::core::{
    activity::ActivitySeverity,
    app::ServerContext,
    packet::Packet,
    router::PacketReply,
    tasks::{TaskResultPayload, TaskStatus},
};

/// Handles a task result packet and records the completed task outcome.
///
/// @param context Shared application state used to store task results.
/// @param packet Parsed packet envelope containing the task result payload.
/// @return Future that resolves to the packet reply sent back to the implant.
pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<TaskResultPayload>() {
            Ok(payload) => match context.record_task_result(payload).await {
                Ok(task) => {
                    context
                        .implants()
                        .set_active_task(task.clientid, None)
                        .await;
                    context
                        .record_activity(
                            match task.status {
                                TaskStatus::Completed => ActivitySeverity::Success,
                                _ => ActivitySeverity::Error,
                            },
                            format!("completed {} for {}", task.task_kind, task.clientid),
                            Some(task.clientid),
                            Some(task.task_id),
                        )
                        .await;
                    PacketReply::packet(
                        StatusCode::OK,
                        packet.opcode_kind(),
                        packet.clientid(),
                        &json!({
                            "status": "ok",
                            "task_id": task.task_id,
                        }),
                    )
                }
                Err(err) => error_reply(&packet, StatusCode::NOT_FOUND, &err.to_string()),
            },
            Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
        }
    })
}

fn error_reply(packet: &Packet, status: StatusCode, message: &str) -> PacketReply {
    PacketReply::packet(
        status,
        packet.opcode_kind(),
        packet.clientid(),
        &json!({
            "status": "error",
            "message": message,
        }),
    )
}
