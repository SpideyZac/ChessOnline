mod tcp_server;

use std::sync::Arc;

use tcp_server::{ServerEvent, TCPServer};

#[tokio::main]
async fn main() {
    let (server, mut events) = TCPServer::new();
    let server = Arc::new(server);

    let server_task = Arc::clone(&server).start("127.0.0.1:8080");

    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                ServerEvent::ClientConnected { addr } => {
                    println!("Client connected: {}", addr);
                }
                ServerEvent::ClientDisconnected { addr } => {
                    println!("Client disconnected: {}", addr);
                }
                ServerEvent::ClientDataReceived { addr, data } => {
                    println!("Data received from {}: {:?}", addr, data);
                }
            }
        }
    });

    if let Err(e) = server_task.await {
        eprintln!("Server error: {}", e);
    }
}
