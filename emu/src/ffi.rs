//! C-compatible FFI layer for iOS integration.
//!
//! This module provides extern "C" functions that can be called from Swift
//! via a bridging header, similar to JNI for Java.

use std::ffi::c_void;
use std::ptr;
use std::slice;

use crate::bus::MemoryBus;
use crate::cpu::Cpu;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::memory::Memory;
use crate::ppu::Ppu;
use crate::timer::Timer;

const CYCLES_PER_FRAME: u32 = 70224;

/// Opaque GameBoy emulator handle for FFI.
pub struct GameBoyHandle {
    cpu: Cpu,
    memory: Memory,
    ppu: Ppu,
    timer: Timer,
    interrupts: InterruptController,
    joypad: Joypad,
    frame_buffer: Box<[u8; 160 * 144 * 4]>,
    frame_count: u32,
}

impl GameBoyHandle {
    fn new() -> Self {
        GameBoyHandle {
            cpu: Cpu::new(),
            memory: Memory::new(),
            ppu: Ppu::new(),
            timer: Timer::new(),
            interrupts: InterruptController::new(),
            joypad: Joypad::new(),
            frame_buffer: Box::new([0; 160 * 144 * 4]),
            frame_count: 0,
        }
    }

    fn load_rom(&mut self, data: &[u8]) -> bool {
        self.memory.load_rom(data).is_ok()
    }

    fn step_frame(&mut self) {
        let mut cycles_elapsed: u32 = 0;

        while cycles_elapsed < CYCLES_PER_FRAME {
            let cycles = {
                let mut bus = MemoryBus::new(&mut self.memory, &mut self.timer, &mut self.joypad);
                self.cpu.step(&mut bus, &mut self.interrupts)
            };

            self.timer.tick(cycles, &mut self.memory, &self.interrupts);
            self.ppu.tick(cycles, &mut self.memory, &self.interrupts);

            cycles_elapsed += cycles;
        }

        self.frame_count += 1;
        self.render_frame();
    }

    fn render_frame(&mut self) {
        let ppu_buffer = self.ppu.get_buffer();
        let palette = [0xFFu8, 0xAA, 0x55, 0x00];

        for (i, &pixel) in ppu_buffer.iter().enumerate() {
            let gray = palette[(pixel & 0x03) as usize];
            let offset = i * 4;
            self.frame_buffer[offset] = gray;     // R
            self.frame_buffer[offset + 1] = gray; // G
            self.frame_buffer[offset + 2] = gray; // B
            self.frame_buffer[offset + 3] = 255;  // A
        }
    }

    fn set_button(&mut self, button: u8, pressed: bool) {
        self.joypad.set_button(button, pressed);
        if pressed {
            self.interrupts.request_joypad(&mut self.memory);
        }
    }

    fn set_camera_image(&mut self, data: &[u8]) {
        self.memory.set_camera_image(data);
    }

    fn is_camera_cartridge(&self) -> bool {
        matches!(self.memory.get_mbc_type(), crate::memory::MbcType::PocketCamera)
    }
}

// ============================================================================
// C FFI Functions
// ============================================================================

/// Create a new GameBoy emulator instance.
/// Returns an opaque pointer that must be freed with `gb_destroy`.
#[unsafe(no_mangle)]
pub extern "C" fn gb_create() -> *mut c_void {
    let handle = Box::new(GameBoyHandle::new());
    Box::into_raw(handle) as *mut c_void
}

/// Destroy a GameBoy emulator instance.
/// The pointer must have been created by `gb_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gb_destroy(handle: *mut c_void) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut GameBoyHandle);
        }
    }
}

/// Load a ROM into the emulator.
/// Returns true on success, false on failure.
#[unsafe(no_mangle)]
pub extern "C" fn gb_load_rom(handle: *mut c_void, data: *const u8, len: usize) -> bool {
    if handle.is_null() || data.is_null() || len == 0 {
        return false;
    }

    unsafe {
        let gb = &mut *(handle as *mut GameBoyHandle);
        let rom_data = slice::from_raw_parts(data, len);
        gb.load_rom(rom_data)
    }
}

/// Run one frame of emulation (~16.74ms of Game Boy time).
#[unsafe(no_mangle)]
pub extern "C" fn gb_step_frame(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    unsafe {
        let gb = &mut *(handle as *mut GameBoyHandle);
        gb.step_frame();
    }
}

/// Get a pointer to the frame buffer (160x144 RGBA pixels).
/// The buffer is owned by the emulator and valid until the next call or destruction.
/// Returns NULL if handle is invalid.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_frame_buffer(handle: *const c_void) -> *const u8 {
    if handle.is_null() {
        return ptr::null();
    }

    unsafe {
        let gb = &*(handle as *const GameBoyHandle);
        gb.frame_buffer.as_ptr()
    }
}

/// Get the frame buffer size in bytes (always 160 * 144 * 4 = 92160).
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_frame_buffer_size() -> usize {
    160 * 144 * 4
}

/// Get the screen width in pixels.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_screen_width() -> u32 {
    160
}

/// Get the screen height in pixels.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_screen_height() -> u32 {
    144
}

/// Set button state.
/// Button values: 0=A, 1=B, 2=Select, 3=Start, 4=Right, 5=Left, 6=Up, 7=Down
#[unsafe(no_mangle)]
pub extern "C" fn gb_set_button(handle: *mut c_void, button: u8, pressed: bool) {
    if handle.is_null() || button > 7 {
        return;
    }

    unsafe {
        let gb = &mut *(handle as *mut GameBoyHandle);
        gb.set_button(button, pressed);
    }
}

/// Set camera image data for Game Boy Camera emulation.
/// Expects 128x112 pixels as 8-bit grayscale (0=black, 255=white).
#[unsafe(no_mangle)]
pub extern "C" fn gb_set_camera_image(handle: *mut c_void, data: *const u8, len: usize) {
    if handle.is_null() || data.is_null() {
        return;
    }

    // Expected size: 128 * 112 = 14336 bytes
    let expected_len = 128 * 112;
    if len < expected_len {
        return;
    }

    unsafe {
        let gb = &mut *(handle as *mut GameBoyHandle);
        let image_data = slice::from_raw_parts(data, expected_len);
        gb.set_camera_image(image_data);
    }
}

/// Check if the loaded ROM is a Game Boy Camera cartridge.
#[unsafe(no_mangle)]
pub extern "C" fn gb_is_camera_cartridge(handle: *const c_void) -> bool {
    if handle.is_null() {
        return false;
    }

    unsafe {
        let gb = &*(handle as *const GameBoyHandle);
        gb.is_camera_cartridge()
    }
}

/// Get the current frame count.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_frame_count(handle: *const c_void) -> u32 {
    if handle.is_null() {
        return 0;
    }

    unsafe {
        let gb = &*(handle as *const GameBoyHandle);
        gb.frame_count
    }
}

/// Get cartridge RAM (save data) size.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_save_size(handle: *const c_void) -> usize {
    if handle.is_null() {
        return 0;
    }

    unsafe {
        let gb = &*(handle as *const GameBoyHandle);
        gb.memory.get_cartridge_ram().len()
    }
}

/// Copy cartridge RAM (save data) to the provided buffer.
/// Returns the number of bytes copied, or 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn gb_get_save_data(handle: *const c_void, buffer: *mut u8, buffer_len: usize) -> usize {
    if handle.is_null() || buffer.is_null() {
        return 0;
    }

    unsafe {
        let gb = &*(handle as *const GameBoyHandle);
        let ram = gb.memory.get_cartridge_ram();
        let copy_len = ram.len().min(buffer_len);

        if copy_len > 0 {
            ptr::copy_nonoverlapping(ram.as_ptr(), buffer, copy_len);
        }

        copy_len
    }
}

/// Load cartridge RAM (save data) from the provided buffer.
/// Returns true on success.
#[unsafe(no_mangle)]
pub extern "C" fn gb_load_save_data(handle: *mut c_void, data: *const u8, len: usize) -> bool {
    if handle.is_null() || data.is_null() || len == 0 {
        return false;
    }

    unsafe {
        let gb = &mut *(handle as *mut GameBoyHandle);
        let save_data = slice::from_raw_parts(data, len);
        gb.memory.load_cartridge_ram(save_data);
        true
    }
}

// Button constants for Swift
pub const GB_BUTTON_A: u8 = 0;
pub const GB_BUTTON_B: u8 = 1;
pub const GB_BUTTON_SELECT: u8 = 2;
pub const GB_BUTTON_START: u8 = 3;
pub const GB_BUTTON_RIGHT: u8 = 4;
pub const GB_BUTTON_LEFT: u8 = 5;
pub const GB_BUTTON_UP: u8 = 6;
pub const GB_BUTTON_DOWN: u8 = 7;

// Screen dimensions
pub const GB_SCREEN_WIDTH: u32 = 160;
pub const GB_SCREEN_HEIGHT: u32 = 144;

// Camera dimensions
pub const GB_CAMERA_WIDTH: u32 = 128;
pub const GB_CAMERA_HEIGHT: u32 = 112;
