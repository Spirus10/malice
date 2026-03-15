use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use hyper::StatusCode;
use serde_json::json;

use crate::util::{
    app::ServerContext,
    packet::{Packet, PacketOpcode},
};

use super::{handlers, reply::PacketReply};

type HandlerFuture = Pin<Box<dyn Future<Output = PacketReply> + Send>>;
type PacketHandler = fn(Arc<ServerContext>, Packet) -> HandlerFuture;

#[derive(Clone)]
pub struct PacketRouter {
    context: Weak<ServerContext>,
    handlers: Arc<HashMap<PacketOpcode, PacketHandler>>,
}

impl PacketRouter {
    pub fn new(context: Weak<ServerContext>) -> Self {
        let mut handlers: HashMap<PacketOpcode, PacketHandler> = HashMap::new();
        handlers.insert(PacketOpcode::Register, handlers::register::handle);
        handlers.insert(PacketOpcode::Heartbeat, handlers::heartbeat::handle);
        handlers.insert(PacketOpcode::FetchTask, handlers::fetch_task::handle);
        handlers.insert(PacketOpcode::TaskResult, handlers::task_result::handle);

        Self {
            context,
            handlers: Arc::new(handlers),
        }
    }

    pub async fn route(&self, packet: Packet) -> PacketReply {
        let Some(context) = self.context.upgrade() else {
            return PacketReply::text(StatusCode::INTERNAL_SERVER_ERROR, "Server context unavailable");
        };

        match self.handlers.get(&packet.opcode_kind()) {
            Some(handler) => handler(context, packet).await,
            None => PacketReply::packet(
                StatusCode::BAD_REQUEST,
                packet.opcode_kind(),
                packet.clientid(),
                &json!({
                    "status": "error",
                    "message": "Unknown opcode",
                }),
            ),
        }
    }
}
