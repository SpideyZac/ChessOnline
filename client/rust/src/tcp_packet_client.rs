use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bincode;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::tcp_client::{ClientEvent, TCPClient};

pub trait Packet: Send + Sync {
    fn packet_type() -> &'static str
    where
        Self: Sized;
    fn packet_type_self(&self) -> &'static str;

    fn encode(&self) -> Vec<u8>;

    fn handler(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}

#[derive(Serialize, Deserialize)]
struct PacketWrapper {
    packet_type: String,
    data: Vec<u8>,
}

pub struct TCPPacketClient {
    client: Arc<TCPClient>,
    event_receiver: Mutex<Receiver<ClientEvent>>,
    registry: HashMap<String, Box<dyn Fn(Vec<u8>) -> Box<dyn Packet>>>,
}

impl TCPPacketClient {
    pub fn new(server_addr: SocketAddr) -> Self {
        let (client, event_receiver) = TCPClient::new(server_addr);
        let client = Arc::new(client);

        TCPPacketClient {
            client,
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

    pub async fn connect(self: Arc<Self>) {
        let client_clone = Arc::clone(&self.client);
        tokio::spawn(async move {
            if let Err(e) = client_clone.connect().await {
                eprintln!("TCP Client error: {}", e);
            }
        });

        self.listen_for_events().await;
    }

    fn create_wrapper(packet: &dyn Packet) -> Vec<u8> {
        let packet_type = packet.packet_type_self().to_string();
        let data = packet.encode();
        let packet_wrapper = PacketWrapper { packet_type, data };
        bincode::serialize(&packet_wrapper).unwrap()
    }

    pub async fn send_packet(&self, packet: &dyn Packet) -> tokio::io::Result<()> {
        let packet_wrapper = Self::create_wrapper(packet);
        self.client.send(packet_wrapper.as_slice()).await
    }

    async fn listen_for_events(self: Arc<Self>) {
        while let Ok(event) = self.event_receiver.lock().unwrap().recv().await {
            match event {
                ClientEvent::Connected => {
                    println!("TCP Packet Client: Connected to server.");
                }
                ClientEvent::Disconnected => {
                    println!("TCP Packet Client: Disconnected from server.");
                }
                ClientEvent::DataReceived { data } => {
                    if let Ok(packet_wrapper) = bincode::deserialize::<PacketWrapper>(&data) {
                        if let Some(deserializer) = self.registry.get(&packet_wrapper.packet_type) {
                            let packet = deserializer(packet_wrapper.data);
                            println!(
                                "TCP Packet Client: Received packet {}",
                                packet.packet_type_self()
                            );
                            tokio::spawn(packet.handler());
                        } else {
                            println!(
                                "TCP Packet Client: Unknown packet type received: {}",
                                packet_wrapper.packet_type
                            );
                        }
                    } else {
                        println!("TCP Packet Client: Invalid data received: {:?}", data);
                    }
                }
            }
        }
    }
}
