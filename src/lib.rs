pub mod events;
pub mod framebuffer;
pub mod leds;
pub mod macros;
pub mod render;
pub mod telemetry;
pub mod transport;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
