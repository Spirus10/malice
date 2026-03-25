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
    activity::ActivitySeverity, app::ServerContext, implants::HeartbeatPayload, packet::Packet,
    router::PacketReply,
};

/// Handles an implant heartbeat packet.
///
/// Flow:
///
///   packet
///      -> parse HeartbeatPayload
///      -> parse clientid from envelope
///      -> update implant liveness/state
///      -> record heartbeat activity
///      -> reply ok
///
/// @param context Shared application state used to update implant liveness.
/// @param packet Parsed packet envelope containing the heartbeat payload.
/// @return Future that resolves to the packet reply sent back to the implant.
pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<HeartbeatPayload>() {
            Ok(payload) => match parse_clientid(packet.clientid()) {
                Ok(clientid) => {
                    match context.implants().update_heartbeat(clientid, payload).await {
                        Ok(record) => {
                            context
                                .record_activity(
                                    ActivitySeverity::Info,
                                    format!(
                                        "heartbeat {} status={}",
                                        record.identity.clientid,
                                        record.runtime_state.current_status
                                    ),
                                    Some(record.identity.clientid),
                                    None,
                                )
                                .await;
                            PacketReply::packet(
                                StatusCode::OK,
                                packet.opcode_kind(),
                                packet.clientid(),
                                &json!({ "status": "ok" }),
                            )
                        }
                        Err(err) => error_reply(&packet, StatusCode::NOT_FOUND, &err.to_string()),
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
