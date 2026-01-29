//! Game Boy emulator core.
//!
//! This crate implements a cycle-accurate Game Boy (DMG) emulator with support
//! for the Game Boy Camera cartridge. It exposes two frontend targets:
//!
//! - **WASM** (`--features wasm`): JavaScript bindings via `wasm-bindgen` for web browsers.
//! - **iOS** (`--features ios`): C FFI functions for Swift integration.
//!
//! Both frontends delegate to `GameBoyCore`, which owns the CPU, memory,
//! PPU, timer, interrupt controller, and joypad.

mod bus;
mod core;
mod cpu;
mod interrupts;
mod joypad;
mod log;
pub(crate) mod memory;
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
