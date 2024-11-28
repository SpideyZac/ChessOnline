mod tcp_packet_wrapper;
mod tcp_server;

use std::sync::Arc;

use tcp_packet_wrapper::TCPPacketWrapper;

#[tokio::main]
async fn main() {
    let packet_wrapper = TCPPacketWrapper::new();
    let packet_wrapper = Arc::new(packet_wrapper);
    packet_wrapper.start("127.0.0.1:8080").await;
}
