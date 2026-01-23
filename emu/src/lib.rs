// Core emulator modules (always compiled)
mod bus;
mod cpu;
mod interrupts;
mod joypad;
mod log;
pub mod memory;
mod ppu;
mod timer;

// FFI module for iOS/native builds
#[cfg(feature = "ios")]
pub mod ffi;

// WASM module for web builds
#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "wasm")]
pub use wasm::*;
