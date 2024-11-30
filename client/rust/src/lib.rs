mod tcp_client;
mod tcp_packet_client;

use godot::prelude::*;

struct Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Extension {}
