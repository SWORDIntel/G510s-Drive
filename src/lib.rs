pub mod alerts;
pub mod audio;
pub mod events;
pub mod framebuffer;
pub mod ipc;
pub mod leds;
pub mod macros;
pub mod plugins;
pub mod render;
pub mod telemetry;
pub mod transport;
pub mod tui;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
