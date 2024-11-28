use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bincode;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::tcp_server::{ServerEvent, TCPServer};

pub trait Packet: Send + Sync {
    fn packet_type() -> &'static str
    where
        Self: Sized;
    fn packet_type_self(&self) -> &'static str;

    fn encode(&self) -> Vec<u8>;

    fn handler(
        &self,
        addr: SocketAddr,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}

#[derive(Serialize, Deserialize)]
struct PacketWrapper {
    packet_type: String,
    data: Vec<u8>,
}

pub struct TCPPacketWrapper {
    server: Arc<TCPServer>,
    event_receiver: Mutex<Receiver<ServerEvent>>,
    registry: HashMap<String, Box<dyn Fn(Vec<u8>) -> Box<dyn Packet>>>,
}

impl TCPPacketWrapper {
    pub fn new() -> Self {
        let (server, event_receiver) = TCPServer::new();
        let server = Arc::new(server);

        TCPPacketWrapper {
            server,
            event_receiver: Mutex::new(event_receiver),
            registry: HashMap::new(),
        }
    }

    pub fn register_packet_type<P>(&mut self)
    where
        P: Packet + Serialize + for<'de> Deserialize<'de> + 'static,
    {
        self.registry.insert(
            P::packet_type().to_string(),
            Box::new(|data| {
                let packet: P = bincode::deserialize(&data).unwrap();
                Box::new(packet)
            }),
        );
    }

    pub async fn start(self: Arc<Self>, addr: &str) {
        let server_clone = Arc::clone(&self.server);
        let addr_clone = addr.to_string();
        tokio::spawn(async move {
            if let Err(e) = server_clone.start(addr_clone.as_str()).await {
                eprintln!("TCP Server error: {}", e);
            }
        });

        self.listen_for_events().await;
    }

    pub async fn send_packet(
        &self,
        addr: &SocketAddr,
        packet: &dyn Packet,
    ) -> tokio::io::Result<()> {
        let packet_wrapper = PacketWrapper {
            packet_type: packet.packet_type_self().to_string(),
            data: packet.encode(),
        };

        let data = bincode::serialize(&packet_wrapper).map_err(|e| {
            tokio::io::Error::new(
                tokio::io::ErrorKind::InvalidData,
                format!("Serialization error: {}", e),
            )
        })?;
        self.server.send_to(addr, &data).await
    }

    async fn listen_for_events(self: Arc<Self>) {
        while let Ok(event) = self.event_receiver.lock().unwrap().recv().await {
            match event {
                ServerEvent::ClientConnected { addr } => {
                    println!("TCP Packet Wrapper: Client connected: {}", addr);
                }
                ServerEvent::ClientDisconnected { addr } => {
                    println!("TCP Packet Wrapper: Client disconnected: {}", addr);
                }
                ServerEvent::ClientDataReceived { addr, data } => {
                    if let Ok(packet_wrapper) = bincode::deserialize::<PacketWrapper>(&data) {
                        if let Some(deserializer) = self.registry.get(&packet_wrapper.packet_type) {
                            let packet = deserializer(packet_wrapper.data);
                            println!(
                                "TCP Packet Wrapper: Received packet from {} {}",
                                addr,
                                packet.packet_type_self()
                            );
                            tokio::spawn(packet.handler(addr));
                        } else {
                            println!(
                                "TCP Packet Wrapper: Unknown packet type received: {}",
                                packet_wrapper.packet_type
                            );
                        }
                    } else {
                        println!(
                            "TCP Packet Wrapper: Invalid data received from {}: {:?}",
                            addr, data
                        );
                    }
                }
            }
        }
    }
}
