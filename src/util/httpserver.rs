use std::{
    collections::HashMap, convert::Infallible, net::{Incoming, SocketAddr}, 
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    clone::Clone,
    io::Write,
};

use http_body_util::{BodyExt, Full};
use hyper::body::{Buf, Bytes};
use hyper::server::conn::http1;
use hyper::service::{service_fn, Service};
use hyper::{body::Incoming as IncomingBody, Request, Response, Method};
use hyper_util::rt::TokioIo;

use tokio::{
    sync::Mutex,
    net::{
        TcpListener,
        TcpStream,
    },
    task,
    io::{
        self,
        AsyncWriteExt as _,
    },
};

use uuid::Uuid;

use serde_json::{Value, json};
use warp::test;

use crate::util::packet;

use super::{
    tasks::TaskManager,
    packet::{Packet, PacketOpcode},
};
pub struct HttpServer {

     local_addr: SocketAddr,
     closed: Arc<AtomicBool>,
     connections: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
     t_manager: TaskManager,
     handle: Arc<Mutex<Option<task::JoinHandle<()>>>>,

}

impl Clone for HttpServer {
    fn clone(&self) -> Self {
        Self {
            local_addr: self.local_addr.clone(),
            closed: Arc::clone(&self.closed),
            connections: Arc::clone(&self.connections),
            t_manager: self.t_manager.clone(),
            handle: Arc::clone(&self.handle),
        }
    }
}

impl HttpServer {

    // Handles routing based on uri path
    pub async fn handle_request<
    CB: 'static + Fn(Packet) + Send + Copy
    >(
        server: &HttpServer, 
        req: Request<IncomingBody>, 
        peer_addr: SocketAddr,
        cb: CB
    ) -> Result<Response<Full<Bytes>>, Infallible> {

        // println!("{:#?}", req);

        let endpoint = req.uri().path();

        // println!("{}", peer_addr);

        match endpoint {

            "/tasks" => {

                match req.method() {
                    &Method::GET => {

                        let mut buf = req.collect().await.unwrap().to_bytes().to_vec();

                        // Prepend opcode based on api endpoint
                        buf.insert(0, PacketOpcode::FETCH_TASK.to_u8());

                        match Packet::new(peer_addr, &buf) {
                            Ok(p) => {
                                println!("{:#?}", p);
                                cb(p);
                            },
                            Err(e) => eprintln!("Error parsing bytes into packet: {}", e),
                        }

                        // let json: Value = serde_json::from_str(String::from_utf8(bytes.to_vec()).unwrap().as_str()).unwrap();

                        // // println!("{}", json);

                        // if let Some(id) = json.get("agent-id") {


                        //     let id_string = id.to_string();
                            
                        //     match Uuid::parse_str(&id_string.trim_matches('"').trim()) {
                        //         Ok(id) => { 
                        //             // println!("Successfully parsed Uuid: {}", id_string);

                        //             // _c will be used eventually to verify that the implant downloaded the correct number of tasks
                        //             // before the server deletes them from the queue
                        //             let (_c, task_result) = server.task_get(&id).await;

                        //             match task_result {

                        //                 Ok(r) => return Ok(r),
                        //                 Err(e) => {
                        //                     eprintln!("Error fetching tasks: {e}");
                        //                     return Ok(Response::new(Full::new(Bytes::from("[]")))); 
                        //                 }
                        //             }
                        //         },
                        //         Err(e) => { 
                        //             eprintln!("Error parsing Uuid! {e}");
                        //             return Ok(Response::new(Full::new(Bytes::from("[]")))); 
                        //         },
                        //     }

                        // } else {
                        //     return Ok(Response::new(Full::new(Bytes::from("Incorrectly formatted JSON"))));
                        // }

                        

                        // println!("{}", json);

                        return Ok(Response::new(Full::new(Bytes::from("[]"))));
                    },
                    &Method::POST => {
                        return Ok(Response::new(Full::new(Bytes::from("[]"))));
                    },
                    _ => {
                        return Ok(Response::new(Full::new(Bytes::from("METHOD NOT ALLOWED"))));
                    },
                }
            },
            "/echo" => {
                println!("{:#?}", req);
                return Ok(Response::new(Full::new(Bytes::from("[]"))))
            }

            _ => return Ok(Response::new(Full::new(Bytes::from("Not a valid endpoint!")))),

        }
        // Ok(Response::new(Full::new(Bytes::from("uh oh fucky aaaaa!!!!"))))

    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        if !self.closed.load(Ordering::Relaxed) {
            println!("HttpServer already running.");
            return Ok(())
        }

        self.closed.store(false, Ordering::Relaxed);

        let listener = TcpListener::bind(self.local_addr).await?;

        let server_arc = Arc::new(Mutex::new(self.clone()));

        let server_clone = Arc::clone(&server_arc);

        server_clone.lock().await.populate_test_tasks().await;

        let handle = tokio::task::spawn(async move {
            println!("ws main thread spawned");
            loop {

                let (stream, _) = listener.accept().await.unwrap();

                let peer_addr = stream.peer_addr().unwrap();

                let io = TokioIo::new(stream);
                
                // Clone the Arc again for the service function
                let server_clone = Arc::clone(&server_arc);
                
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(move |req| {
                        let server_clone = Arc::clone(&server_clone); // Clone for each request
                        async move {

                            // Lock the mutex to access server instance
                            let server = server_clone.lock().await;

                            // Check for shutdown flag
                            // if server.closed.load(Ordering::Relaxed) {
                            //     let mut lock = server.handle.lock().await;
                            //     match lock.take() {
                            //         Some(h) => {
                            //             h.abort();
                            //             *lock = None;
                            //         },
                            //         None => {
                            //             eprintln!("HttpServer is not running.... should be unreachable");
                            //         }
                            //     }
                            // }

                            HttpServer::handle_request(&*server, req, peer_addr, packet::packet_cb).await
                        }
                    }))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            }
        });

        *self.handle.lock().await = Some(handle);

        // // Check for shutdown flag
        // if self.closed.load(Ordering::Relaxed) {
        //     let mut lock = self.handle.lock().await;
        //     match lock.take() {
        //         Some(h) => {
        //             h.abort();
        //             *lock = None;
        //         },
        //         None => {
        //             eprintln!("HttpServer is not running.... should be unreachable");
        //         }
        //     }
        // }

        Ok(())
    }


    pub async fn close(&mut self) {

        let handle_clone = Arc::clone(&self.handle);

        let mut lock = handle_clone.lock().await;

        self.closed.store(true, Ordering::Relaxed);

        match lock.take() {
            Some(h) => {
                h.abort();
                *lock = None;
            },
            None => {
                eprintln!("HttpServer is not running... ");
            }
        }
    }

    pub async fn new () -> std::io::Result<Self> {

        Ok(Self { 
            local_addr: SocketAddr::from(([127, 0, 0, 1], 42069)), 
            closed: Arc::new(AtomicBool::new(true)), 
            connections: Arc::new(Mutex::new(HashMap::new())), 
            t_manager: TaskManager::new(),
            handle: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn task_post(&self, uuid: &Uuid) -> Result<Response<Full<Bytes>>, Infallible> {
        todo!();
    }

    pub async fn task_get(&self, uuid: &Uuid) -> (u8, Result<Response<Full<Bytes>>, Infallible>) {
        
        if let Some( (c, v)) = self.t_manager.get_tasks(uuid).await {
            let joined = v.join("");
            (c, Ok(Response::new(Full::new(Bytes::from(joined)))))
        } else {
            (0, Ok(Response::new(Full::new(Bytes::from("No tasks found for that uuid!")))))
        }
    }

    async fn populate_test_tasks(&self) {
        let test_uuid = Uuid::new_v4();

        println!("Test uuid: {}", test_uuid);
        self.t_manager.populate_test_entry(&test_uuid).await;
    }

}

#[cfg(test)]
mod tests {

    use hyper::client::conn::http1::SendRequest;

    use super::*;

    pub struct TestClient {
        sender: SendRequest<http_body_util::Empty<Bytes>>,
        conn: hyper::client::conn::http1::Connection<TokioIo<TcpStream>, http_body_util::Empty<Bytes>>,
    }

    impl TestClient {
        pub async fn new() -> Self {
            let url: hyper::Uri = hyper::Uri::from_static("http://127.0.0.1:42069");

            let host = url.host().expect("uri has no host");
            let port = url.port_u16().unwrap_or(42069);

            let addr = format!("{}:{}",  host, port);

            let stream = TcpStream::connect(addr).await.unwrap();

            let io = TokioIo::new(stream);

            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();

            Self{
                sender: sender,
                conn: conn,
            }

        }
    }

    #[tokio::test]
    async fn test_task_get_api() {

        let url: hyper::Uri = hyper::Uri::from_static("http://127.0.0.1:42069");

        let mut http_server = HttpServer::new().await.unwrap();

        if let Err(e) = http_server.start().await {
            eprintln!("Error starting Http Server: {e}");
        }

        let mut client = TestClient::new().await;

        tokio::task::spawn(async move {
            if let Err(err) = client.conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        

        let authority = url.authority().unwrap().clone();

        let req = Request::builder()
            .uri("/echo")
            .header(hyper::header::HOST,  authority.as_str())
            .body(http_body_util::Empty::<Bytes>::new()).unwrap();


        let mut res = client.sender.send_request(req).await.unwrap();

        println!("{:#?}", res);

        assert_eq!(1, 1);
    }

}