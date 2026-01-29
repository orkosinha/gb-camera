//! WASM bindings for web-based emulator.

use wasm_bindgen::prelude::*;

use crate::bus::MemoryBus;
use crate::cpu::Cpu;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::log::LogCategory;
use crate::memory::Memory;
use crate::ppu::Ppu;
use crate::timer::Timer;
use crate::{log_info, log_warn};

const CYCLES_PER_FRAME: u32 = 70224; // ~59.73 Hz

/// Initialize panic hook for better error messages in WASM.
/// This is called once when the WASM module is instantiated.
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct GameBoy {
    cpu: Cpu,
    memory: Memory,
    ppu: Ppu,
    timer: Timer,
    interrupts: InterruptController,
    joypad: Joypad,
    frame_buffer: Box<[u8; 160 * 144 * 4]>,
    camera_live_buffer: Box<[u8; 128 * 112 * 4]>,
    // Debug tracking
    frame_count: u32,
    total_cycles: u64,
    instruction_count: u64,
}

#[wasm_bindgen]
impl GameBoy {
    #[wasm_bindgen(constructor)]
    pub fn new() -> GameBoy {
        log_info!(LogCategory::General, "GameBoy::new() - Creating emulator instance");
        GameBoy {
            cpu: Cpu::new(),
            memory: Memory::new(),
            ppu: Ppu::new(),
            timer: Timer::new(),
            interrupts: InterruptController::new(),
            joypad: Joypad::new(),
            frame_buffer: Box::new([0; 160 * 144 * 4]),
            camera_live_buffer: Box::new([0; 128 * 112 * 4]),
            frame_count: 0,
            total_cycles: 0,
            instruction_count: 0,
        }
    }

    pub fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), JsValue> {
        log_info!(LogCategory::General, "load_rom() - Loading ROM of {} bytes", rom_data.len());

        // Log ROM header information before loading
        if rom_data.len() >= 0x150 {
            let title: String = rom_data[0x134..0x144]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect();
            let cart_type = rom_data[0x147];
            let rom_size = rom_data[0x148];
            let ram_size = rom_data[0x149];

            log_info!(
                LogCategory::General,
                "ROM Header: title='{}', cart_type=0x{:02X}, rom_size=0x{:02X}, ram_size=0x{:02X}",
                title, cart_type, rom_size, ram_size
            );

            // Log entry point bytes
            log_info!(
                LogCategory::General,
                "Entry point (0x100-0x103): {:02X} {:02X} {:02X} {:02X}",
                rom_data[0x100], rom_data[0x101], rom_data[0x102], rom_data[0x103]
            );
        }

        self.memory.load_rom(rom_data).map_err(JsValue::from_str)?;

        // Log ROM info to console
        log_info!(
            LogCategory::General,
            "ROM loaded successfully: {} bytes, MBC: {:?}, Banks: {}",
            rom_data.len(),
            self.memory.get_mbc_type(),
            self.memory.get_rom_bank_count()
        );

        // Log initial state
        log_info!(LogCategory::Cpu, "{}", self.cpu.get_debug_state());
        log_info!(LogCategory::Memory, "{}", self.memory.get_io_state());

        Ok(())
    }

    pub fn step_frame(&mut self) {
        let mut cycles_elapsed: u32 = 0;
        let mut instructions_this_frame: u32 = 0;

        while cycles_elapsed < CYCLES_PER_FRAME {
            let cycles = {
                let mut bus = MemoryBus::new(&mut self.memory, &mut self.timer, &mut self.joypad);
                self.cpu.step(&mut bus, &mut self.interrupts)
            };

            self.timer.tick(cycles, &mut self.memory, &self.interrupts);
            self.ppu.tick(cycles, &mut self.memory, &self.interrupts);

            cycles_elapsed += cycles;
            instructions_this_frame += 1;
            self.instruction_count += 1;
        }

        self.total_cycles += cycles_elapsed as u64;
        self.frame_count += 1;

        // Log state every 60 frames (approximately once per second)
        if self.frame_count % 60 == 1 {
            self.log_frame_debug(instructions_this_frame);
        }

        self.render_frame();
    }

    pub fn frame_buffer_ptr(&self) -> *const u8 {
        self.frame_buffer.as_ptr()
    }

    pub fn frame_buffer_len(&self) -> usize {
        self.frame_buffer.len()
    }

    fn render_frame(&mut self) {
        let ppu_buffer = self.ppu.get_buffer();
        let palette = [0xFFu8, 0xAA, 0x55, 0x00]; // White to black

        for (i, &pixel) in ppu_buffer.iter().enumerate() {
            let gray = palette[(pixel & 0x03) as usize];
            let offset = i * 4;
            self.frame_buffer[offset] = gray; // R
            self.frame_buffer[offset + 1] = gray; // G
            self.frame_buffer[offset + 2] = gray; // B
            self.frame_buffer[offset + 3] = 255; // A
        }
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.joypad.set_button(button, pressed);
        if pressed {
            self.interrupts.request_joypad(&mut self.memory);
        }
    }

    pub fn get_cartridge_ram(&self) -> Vec<u8> {
        self.memory.get_cartridge_ram()
    }

    pub fn load_cartridge_ram(&mut self, data: &[u8]) {
        self.memory.load_cartridge_ram(data);
    }

    /// Set camera image data from webcam.
    /// Expects 128x112 pixels as raw 8-bit grayscale (0=black, 255=white).
    /// Sensor emulation will process this during capture with exposure, gain, and dithering.
    pub fn set_camera_image(&mut self, data: &[u8]) {
        // Pass raw 8-bit grayscale directly to memory
        // Sensor emulation in process_camera_capture() handles the full conversion
        self.memory.set_camera_image(data);
    }

    /// Check if camera image is ready for capture.
    pub fn is_camera_ready(&self) -> bool {
        self.memory.is_camera_image_ready()
    }

    /// Check if the loaded ROM is a Game Boy Camera cartridge.
    pub fn is_camera(&self) -> bool {
        self.memory.get_mbc_type() == crate::memory::MbcType::PocketCamera
    }

    /// Update the camera live view buffer if the capture has changed.
    /// Returns true if the buffer was updated.
    pub fn update_camera_live(&mut self) -> bool {
        if !self.memory.is_camera_capture_dirty() {
            return false;
        }
        self.memory.clear_camera_capture_dirty();

        let sram = self.memory.camera_capture_sram();
        let palette: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
        let buf = &mut *self.camera_live_buffer;

        for tile_y in 0..14 {
            for tile_x in 0..16 {
                let tile_offset = (tile_y * 16 + tile_x) * 16;
                for row in 0..8 {
                    let low = sram[tile_offset + row * 2];
                    let high = sram[tile_offset + row * 2 + 1];
                    for col in 0..8 {
                        let bit = 7 - col;
                        let color_idx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);
                        let gray = palette[color_idx as usize];
                        let px = tile_x * 8 + col;
                        let py = tile_y * 8 + row;
                        let i = (py * 128 + px) * 4;
                        buf[i] = gray;
                        buf[i + 1] = gray;
                        buf[i + 2] = gray;
                        buf[i + 3] = 255;
                    }
                }
            }
        }
        true
    }

    /// Pointer to the camera live view RGBA buffer (128×112×4 bytes).
    pub fn camera_live_ptr(&self) -> *const u8 {
        self.camera_live_buffer.as_ptr()
    }

    /// Length of the camera live view buffer.
    pub fn camera_live_len(&self) -> usize {
        self.camera_live_buffer.len()
    }

    /// Decode a GB Camera saved photo slot to RGBA pixel data.
    /// Slots 1-30 = saved photos. Returns empty if slot is unoccupied.
    pub fn decode_camera_photo(&self, slot: u8) -> Vec<u8> {
        self.memory.decode_camera_photo(slot)
    }

    /// Get serial output as a string (for test ROM debugging).
    pub fn get_serial_output(&self) -> String {
        self.memory.get_serial_output_string()
    }

    /// Clear the serial output buffer.
    pub fn clear_serial_output(&mut self) {
        self.memory.clear_serial_output();
    }

    /// Get debug info about the emulator state and log to console.
    pub fn get_debug_info(&self) -> String {
        let info = format!(
            "MBC: {:?}, ROM banks: {}, LCDC: 0x{:02X}, LY: {}",
            self.memory.get_mbc_type(),
            self.memory.get_rom_bank_count(),
            self.memory.read_io_direct(0x40),
            self.memory.read_io_direct(0x44),
        );
        log_info!(LogCategory::General, "{}", info);
        info
    }

    /// Log a message to the browser console.
    pub fn log(&self, msg: &str) {
        log_info!(LogCategory::General, "{}", msg);
    }

    /// Log frame debug info.
    fn log_frame_debug(&self, instructions_this_frame: u32) {
        log_info!(
            LogCategory::General,
            "=== Frame {} | cycles: {} | instrs: {} (frame: {}) ===",
            self.frame_count, self.total_cycles, self.instruction_count, instructions_this_frame
        );

        // Log component states using Display implementations
        log_info!(LogCategory::Cpu, "{}", self.cpu.get_debug_state());
        log_info!(LogCategory::Ppu, "{}", self.ppu.get_debug_state());
        log_info!(LogCategory::Memory, "{}", self.memory.get_io_state());
        log_info!(LogCategory::Memory, "{}", self.memory.get_debug_state());

        // Check for potential issues
        if !self.memory.is_lcd_enabled() {
            log_warn!(LogCategory::General, "LCD is disabled (LCDC bit 7 = 0)");
        }

        // Log buffer stats
        log_info!(
            LogCategory::Ppu,
            "buffer non-zero pixels: {}",
            self.ppu.count_non_zero_pixels()
        );
    }

    /// Get frame count for debugging.
    pub fn get_frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Get total instruction count for debugging.
    pub fn get_instruction_count(&self) -> u64 {
        self.instruction_count
    }

    /// Log detailed VRAM tile data for debugging.
    pub fn log_vram_info(&self) {
        let lcdc = self.memory.read_io_direct(0x40);
        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x08 != 0 { 0x9C00 } else { 0x9800 };

        log_info!(
            LogCategory::Ppu,
            "VRAM: tile_data={:04X} tile_map={:04X}",
            tile_data_base,
            tile_map_base
        );

        // Log first few tile indices from tile map
        let tile_indices: Vec<String> = (0..16)
            .map(|i| format!("{:02X}", self.memory.read(tile_map_base + i)))
            .collect();
        log_info!(LogCategory::Ppu, "Tile indices: {}", tile_indices.join(" "));

        // Log first tile data
        let tile_data: Vec<String> = (0..16)
            .map(|i| format!("{:02X}", self.memory.read(0x8000 + i)))
            .collect();
        log_info!(LogCategory::Ppu, "Tile 0 data: {}", tile_data.join(" "));
    }
}

impl Default for GameBoy {
    fn default() -> Self {
        Self::new()
    }
}
