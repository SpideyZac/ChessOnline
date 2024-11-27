use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub enum ServerEvent {
    ClientConnected { addr: SocketAddr },
    ClientDisconnected { addr: SocketAddr },
    ClientDataReceived { addr: SocketAddr, data: Vec<u8> },
}

pub struct TCPServer {
    clients: Arc<Mutex<HashMap<SocketAddr, tokio::net::tcp::OwnedWriteHalf>>>,
    event_sender: Sender<ServerEvent>,
}

impl TCPServer {
    pub fn new() -> (Self, Receiver<ServerEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(100);
        (
            TCPServer {
                clients: Arc::new(Mutex::new(HashMap::new())),
                event_sender,
            },
            event_receiver,
        )
    }

    pub async fn start(self: Arc<Self>, addr: &str) -> tokio::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server running on {}", addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    println!("Client connected: {}", addr);
                    self.clone().handle_client(socket, addr).await;
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    async fn handle_client(self: Arc<Self>, socket: TcpStream, addr: SocketAddr) {
        let (mut reader, writer) = socket.into_split();
        self.clients.lock().unwrap().insert(addr, writer);

        let _ = self
            .event_sender
            .send(ServerEvent::ClientConnected { addr });

        let server = Arc::clone(&self);
        tokio::spawn(async move {
            let mut buffer = [0; 1024];
            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Client disconnected: {}", addr);
                        server.clients.lock().unwrap().remove(&addr);
                        let _ = server
                            .event_sender
                            .send(ServerEvent::ClientDisconnected { addr });
                        break;
                    }
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        let _ = server
                            .event_sender
                            .send(ServerEvent::ClientDataReceived { addr, data });
                    }
                    Err(e) => {
                        eprintln!("Error reading from client {}: {}", addr, e);
                        server.clients.lock().unwrap().remove(&addr);
                        let _ = server
                            .event_sender
                            .send(ServerEvent::ClientDisconnected { addr });
                        break;
                    }
                }
            }
        });
    }

    pub async fn send_to(&self, addr: &SocketAddr, data: &[u8]) -> tokio::io::Result<()> {
        if let Some(writer) = self.clients.lock().unwrap().get_mut(addr) {
            writer.write_all(data).await
        } else {
            Err(tokio::io::Error::new(
                tokio::io::ErrorKind::NotFound,
                "Client not found",
            ))
        }
    }

    pub async fn send_to_multiple(&self, addrs: &[SocketAddr], data: &[u8]) {
        for addr in addrs {
            if let Err(e) = self.send_to(addr, data).await {
                eprintln!("Failed to send to {}: {}", addr, e);
            }
        }
    }

    pub async fn broadcast(&self, data: &[u8]) {
        for writer in self.clients.lock().unwrap().values_mut() {
            if let Err(e) = writer.write_all(data).await {
                eprintln!("Broadcast error: {}", e);
            }
        }
    }
}
