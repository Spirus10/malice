use std::{future::Future, pin::Pin, sync::Arc};

use hyper::StatusCode;
use serde_json::json;

use crate::util::{
    activity::ActivitySeverity,
    app::ServerContext,
    packet::Packet,
    router::PacketReply,
    tasks::{TaskResultPayload, TaskStatus},
};

pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<TaskResultPayload>() {
            Ok(payload) => match context.tasks().record_result(payload).await {
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
                            format!("completed {} for {}", task.spec.task_type(), task.clientid),
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
