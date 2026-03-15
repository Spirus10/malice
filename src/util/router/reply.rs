use http_body_util::Full;
use hyper::{
    body::Bytes,
    header::{HeaderValue, CONTENT_TYPE},
    Response, StatusCode,
};
use serde::Serialize;

use crate::util::packet::{Packet, PacketOpcode};

pub struct PacketReply {
    status: StatusCode,
    body: Vec<u8>,
}

impl PacketReply {
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

    pub fn text(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            body: message.into().into_bytes(),
        }
    }

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
