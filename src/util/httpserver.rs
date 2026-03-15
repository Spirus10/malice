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

#[derive(Clone)]
pub struct HttpServer {
    local_addr: SocketAddr,
    closed: Arc<AtomicBool>,
    context: Arc<ServerContext>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl HttpServer {
    pub fn new(context: Arc<ServerContext>) -> Self {
        Self {
            local_addr: SocketAddr::from(([127, 0, 0, 1], 42069)),
            closed: Arc::new(AtomicBool::new(true)),
            context,
            handle: Arc::new(Mutex::new(None)),
        }
    }

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

        let body = match req.collect().await {
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

        Ok(context.router().route(packet).await.into_http_response())
    }

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

    pub async fn close(&mut self) {
        self.closed.store(true, Ordering::Relaxed);

        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }

    pub fn context(&self) -> Arc<ServerContext> {
        self.context.clone()
    }

    pub fn is_running(&self) -> bool {
        !self.closed.load(Ordering::Relaxed)
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}
