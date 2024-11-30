use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    DataReceived { data: Vec<u8> },
}

pub struct TCPClient {
    server_addr: SocketAddr,
    event_sender: Sender<ClientEvent>,
    writer: Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
}

impl TCPClient {
    pub fn new(server_addr: SocketAddr) -> (Self, Receiver<ClientEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(100);
        (
            TCPClient {
                server_addr,
                event_sender,
                writer: Arc::new(Mutex::new(None)),
            },
            event_receiver,
        )
    }

    pub async fn connect(self: Arc<Self>) -> tokio::io::Result<()> {
        match TcpStream::connect(self.server_addr).await {
            Ok(socket) => {
                println!("TCP Client: Connected to server {}", self.server_addr);

                let (mut reader, writer) = socket.into_split();
                *self.writer.lock().unwrap() = Some(writer);

                let _ = self.event_sender.send(ClientEvent::Connected);

                let client = Arc::clone(&self);
                tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    loop {
                        match reader.read(&mut buffer).await {
                            Ok(0) => {
                                println!("TCP Client: Server disconnected");
                                let _ = client.event_sender.send(ClientEvent::Disconnected);
                                break;
                            }
                            Ok(n) => {
                                let data = buffer[..n].to_vec();
                                println!("TCP Client: Received {} bytes from server", n);
                                let _ =
                                    client.event_sender.send(ClientEvent::DataReceived { data });
                            }
                            Err(e) => {
                                eprintln!("TCP Client: Error reading from server: {}", e);
                                let _ = client.event_sender.send(ClientEvent::Disconnected);
                                break;
                            }
                        }
                    }
                });

                Ok(())
            }
            Err(e) => {
                eprintln!("TCP Client: Failed to connect to server: {}", e);
                Err(e)
            }
        }
    }

    pub async fn send(&self, data: &[u8]) -> tokio::io::Result<()> {
        if let Some(writer) = self.writer.lock().unwrap().as_mut() {
            writer.write_all(data).await
        } else {
            Err(tokio::io::Error::new(
                tokio::io::ErrorKind::NotConnected,
                "Not connected to server",
            ))
        }
    }

    pub async fn disconnect(&self) {
        let mut writer_lock = self.writer.lock().unwrap();
        if writer_lock.is_some() {
            println!("TCP Client: Disconnecting from server");
            *writer_lock = None;
            let _ = self.event_sender.send(ClientEvent::Disconnected);
        }
    }
}
