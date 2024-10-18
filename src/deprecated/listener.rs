use std::collections::HashMap;
use std::fmt;
use std::io::{prelude::*, BufReader};
use std::sync::{Arc, Mutex, mpsc};
use std::{net::{TcpListener, TcpStream}, io::Read};
use std::thread;

use crate::logger;

use lazy_static::lazy_static;

use uuid::Uuid;

// Global Listener List
// ==============================================================================================================

// Global listener list
lazy_static! { static ref LISTENER_LIST: Arc<Mutex<HashMap<String, Listener>>> = Arc::new(Mutex::new(HashMap::new())); }

// Append listener to global list
pub fn g_add_listener(listener: Listener) {
    LISTENER_LIST.lock().unwrap().insert(listener.name.clone(), listener);
}

// Remove listener from global list
pub fn g_remove_listener(name: &String) {

    let mut listeners = LISTENER_LIST.lock().unwrap();

    if let Some(mut listener) = listeners.remove(name) {
        // Call shutdown on listener after HashMap no longer owns it
        listener.shutdown();

        logger::info("Successfully removed listener\n");
    } else {
        logger::info("There is no listener with that UUID :(\n")
    }
}

// Display global listener list
pub fn g_display_listeners() {

    let listeners = LISTENER_LIST.lock().unwrap();

    logger::info("--------------- Global Listener List --------------\n\n");
    for (_, listener) in  listeners.iter() {
        println!("{}\n", listener);
    }
}



 // Listener
 // =================================================================================================================================
 pub struct Listener {
    
    id: Uuid,

    name: String,

    address: String,

    port: u16,

    uri: String,

    password: String,

    listener_thread: Option<thread::JoinHandle<()>>,

    shutdown_tx: Option<mpsc::Sender<()>>,

}

impl Listener {

    pub fn new(name: &String, address: &String, port: u16, uri: &String, password: &String) {

        let addr: String;
        if address == "localhost" { 
            addr = "127.0.0.1".to_owned(); 
        } else {
            addr = address.to_string();
        }

        let mut res = Self {
            id: Uuid::new_v4(),
            address: addr.to_string(),
            name: name.to_string(),
            port,
            uri: uri.to_string(),
            password: password.to_string(),
            listener_thread: None,
            shutdown_tx: None,
        };

        res.start();

        // Append listener to global list
        // NOTE: this moves ownership of the listener to the global list.
        g_add_listener(res);

    }

    pub fn start(&mut self) {
        let listener = TcpListener::bind(format!("{}:{}", self.address, self.port)).expect("Unable to bind TcpListener!");
        let listener = Arc::new(Mutex::new(listener));

        let (shutdown_tx, shutdown_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();

        self.shutdown_tx = Some(shutdown_tx);

        let listener_clone = Arc::clone(&listener);

        self.listener_thread = Some(thread::spawn(move || {

            loop {

                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                match listener_clone.lock().unwrap().incoming().next() {

                    Some(Ok(stream)) => {
                        
                        handle_tcp_connection(stream);

                    },

                    Some(Err(e)) => eprintln!("Error accepting connection {:?}", e),

                    None => {
                        thread::yield_now();
                    }
                }
                
            }
        }));
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).expect("Failed to send shutdown signal!");
        }

        println!("Attempting to take thread");

        self.listener_thread
            .take().expect("Called stop on non-running thread")
            .join().expect("Could not join spawned thread");
    }

    
}



impl std::fmt::Display for Listener {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id: {}, Name: {}, Address: {}, Port: {}, Uri: {}, Password: {}", self.id, self.name, self.address, self.port, self.uri, self.password)
    }
}
