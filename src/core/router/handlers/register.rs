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
    activity::ActivitySeverity, app::ServerContext, implants::RegisterPayload, packet::Packet,
    router::PacketReply,
};

/// Handles an implant registration packet.
///
/// @param context Shared application state used to update implant records.
/// @param packet Parsed packet envelope containing the registration payload.
/// @return Future that resolves to the packet reply sent back to the implant.
pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<RegisterPayload>() {
            Ok(payload) => match parse_requested_clientid(packet.clientid()) {
                Ok(requested_clientid) => {
                    match context.register_implant(requested_clientid, payload).await {
                        Ok(record) => {
                            context
                                .record_activity(
                                    ActivitySeverity::Success,
                                    format!(
                                        "registered {} on {}\\{}",
                                        record.identity.clientid,
                                        record.static_metadata.hostname,
                                        record.static_metadata.username
                                    ),
                                    Some(record.identity.clientid),
                                    None,
                                )
                                .await;
                            PacketReply::packet(
                                StatusCode::OK,
                                packet.opcode_kind(),
                                &record.identity.clientid.to_string(),
                                &json!({
                                    "status": "ok",
                                    "clientid": record.identity.clientid,
                                }),
                            )
                        }
                        Err(err) => error_reply(&packet, StatusCode::FORBIDDEN, &err.to_string()),
                    }
                }
                Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
            },
            Err(err) => error_reply(&packet, StatusCode::BAD_REQUEST, &err.to_string()),
        }
    })
}

fn parse_requested_clientid(clientid: &str) -> Result<Option<Uuid>, Error> {
    if clientid.trim().is_empty() {
        return Ok(None);
    }

    Uuid::from_str(clientid)
        .map(Some)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e.to_string()))
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
