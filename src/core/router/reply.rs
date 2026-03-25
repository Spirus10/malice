use http_body_util::Full;
use hyper::{
    body::Bytes,
    header::{HeaderValue, CONTENT_TYPE},
    Response, StatusCode,
};
use serde::Serialize;

use crate::core::packet::{Packet, PacketOpcode};

pub struct PacketReply {
    status: StatusCode,
    body: Vec<u8>,
}

impl PacketReply {
    /// Builds a binary packet reply from a typed payload.
    ///
    /// @param status HTTP status returned alongside the packet body.
    /// @param opcode Packet opcode encoded into the reply body.
    /// @param clientid Implant identifier written into the packet header.
    /// @param payload Serializable payload body.
    /// @return Packet reply containing the encoded payload or an error body.
    pub fn packet<T: Serialize>(
        status: StatusCode,
        opcode: PacketOpcode,
        clientid: &str,
        payload: &T,
    ) -> Self {
        match Packet::build(opcode, clientid, payload) {
            Ok(body) => Self { status, body },
            Err(err) => Self::text(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    }

    /// Builds a plain-text reply body.
    ///
    /// @param status HTTP status returned alongside the message.
    /// @param message Text body returned to the caller.
    /// @return Packet reply containing the message bytes.
    pub fn text(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            body: message.into().into_bytes(),
        }
    }

    /// Converts the internal reply into a Hyper HTTP response.
    ///
    /// @return HTTP response with the packet body and octet-stream content type.
    pub fn into_http_response(self) -> Response<Full<Bytes>> {
        let mut response = Response::new(Full::new(Bytes::from(self.body)));
        *response.status_mut() = self.status;
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        response
    }
}
