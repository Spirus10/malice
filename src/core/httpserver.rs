//! Hyper-based HTTP packet ingress used by the implant runtime.

use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, body::Incoming as IncomingBody, Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::{
    net::TcpListener,
    sync::Mutex,
    task::{self, JoinHandle},
};

use super::{app::ServerContext, packet::Packet};

const REGISTER_HEADER_NAME: &str = "x-malice-register";

#[derive(Clone)]
pub struct HttpServer {
    local_addr: SocketAddr,
    closed: Arc<AtomicBool>,
    context: Arc<ServerContext>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl HttpServer {
    /// Creates an HTTP packet server wrapper around the shared server context.
    ///
    /// @param context Shared application state used by request handlers.
    /// @return HTTP server wrapper bound to the default localhost address.
    pub fn new(context: Arc<ServerContext>) -> Self {
        Self {
            local_addr: SocketAddr::from(([127, 0, 0, 1], 42069)),
            closed: Arc::new(AtomicBool::new(true)),
            context,
            handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Processes one inbound HTTP request and routes valid packet bodies.
    ///
    /// @param context Shared application state used to route and fulfill packets.
    /// @param req Incoming HTTP request from an implant.
    /// @param peer_addr Remote socket address associated with the request.
    /// @return HTTP response containing either a packet reply or a request error.
    pub async fn handle_request(
        context: Arc<ServerContext>,
        req: Request<IncomingBody>,
        peer_addr: SocketAddr,
    ) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
        if req.method() != Method::POST || req.uri().path() != "/packet" {
            let mut response = Response::new(Full::new(Bytes::from("Not found")));
            *response.status_mut() = StatusCode::NOT_FOUND;
            return Ok(response);
        }

        let (parts, body) = req.into_parts();

        let body = match body.collect().await {
            Ok(body) => body.to_bytes(),
            Err(err) => {
                let mut response = Response::new(Full::new(Bytes::from(err.to_string())));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(response);
            }
        };

        let packet = match Packet::new(peer_addr, &body) {
            Ok(packet) => packet,
            Err(err) => {
                let mut response = Response::new(Full::new(Bytes::from(err.to_string())));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(response);
            }
        };

        let request_context = context.packet_request_context(
            parts
                .headers
                .get(REGISTER_HEADER_NAME)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        );
        if let Err(err) = context.admission().validate(&request_context, &packet) {
            let mut response = Response::new(Full::new(Bytes::from(err.to_string())));
            *response.status_mut() = StatusCode::FORBIDDEN;
            return Ok(response);
        }

        Ok(context.router().route(packet).await.into_http_response())
    }

    /// Starts accepting implant traffic on the configured address.
    ///
    /// @return Result describing whether the listener task started successfully.
    pub async fn start(
        &mut self,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.closed.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.closed.store(false, Ordering::Relaxed);

        let listener = TcpListener::bind(self.local_addr).await?;
        let context = self.context.clone();

        let handle = task::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(parts) => parts,
                    Err(_) => break,
                };

                let peer_addr = match stream.peer_addr() {
                    Ok(addr) => addr,
                    Err(_) => continue,
                };

                let io = TokioIo::new(stream);
                let context = context.clone();

                let connection = http1::Builder::new().serve_connection(
                    io,
                    service_fn(move |req| {
                        let context = context.clone();
                        async move { HttpServer::handle_request(context, req, peer_addr).await }
                    }),
                );

                task::spawn(async move {
                    let _ = connection.await;
                });
            }
        });

        *self.handle.lock().await = Some(handle);
        Ok(())
    }

    /// Stops the listener task if it is currently running.
    ///
    /// @return None.
    pub async fn close(&mut self) {
        self.closed.store(true, Ordering::Relaxed);

        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }

    /// Returns the shared server context used by the listener.
    ///
    /// @return Shared server context.
    pub fn context(&self) -> Arc<ServerContext> {
        self.context.clone()
    }

    /// Reports whether the listener task is currently running.
    ///
    /// @return `true` when the server has been started and not yet closed.
    pub fn is_running(&self) -> bool {
        !self.closed.load(Ordering::Relaxed)
    }

    /// Returns the configured local socket address for the server.
    ///
    /// @return Socket address used for listener binding.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}
