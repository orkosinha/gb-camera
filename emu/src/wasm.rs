//! WASM bindings for web-based emulator.
//!
//! All emulation logic lives in [`GameBoyCore`]; this module is a thin
//! adapter that exposes it to JavaScript via `wasm-bindgen`.

use wasm_bindgen::prelude::*;

use crate::core::GameBoyCore;
use crate::log::LogCategory;
use crate::memory::io;
use crate::{log_info, log_warn};

/// Initialize panic hook for better error messages in WASM.
/// This is called once when the WASM module is instantiated.
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct GameBoy {
    core: GameBoyCore,
}

#[wasm_bindgen]
impl GameBoy {
    #[wasm_bindgen(constructor)]
    pub fn new() -> GameBoy {
        log_info!(
            LogCategory::General,
            "GameBoy::new() - Creating emulator instance"
        );
        GameBoy {
            core: GameBoyCore::new(),
        }
    }

    pub fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), JsValue> {
        log_info!(
            LogCategory::General,
            "load_rom() - Loading ROM of {} bytes",
            rom_data.len()
        );

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
                title,
                cart_type,
                rom_size,
                ram_size
            );

            log_info!(
                LogCategory::General,
                "Entry point (0x100-0x103): {:02X} {:02X} {:02X} {:02X}",
                rom_data[0x100],
                rom_data[0x101],
                rom_data[0x102],
                rom_data[0x103]
            );
        }

        self.core.load_rom(rom_data).map_err(JsValue::from_str)?;

        log_info!(
            LogCategory::General,
            "ROM loaded successfully: {} bytes, MBC: {:?}, Banks: {}",
            rom_data.len(),
            self.core.memory.get_mbc_type(),
            self.core.memory.get_rom_bank_count()
        );

        log_info!(LogCategory::Cpu, "{}", self.core.cpu.get_debug_state());
        log_info!(LogCategory::Memory, "{}", self.core.memory.get_io_state());

        Ok(())
    }

    pub fn step_frame(&mut self) {
        let instructions_this_frame = self.core.step_frame();

        // Log state every 60 frames (approximately once per second)
        if self.core.frame_count % 60 == 1 {
            self.log_frame_debug(instructions_this_frame);
        }
    }

    pub fn frame_buffer_ptr(&self) -> *const u8 {
        self.core.frame_buffer.front().as_ptr()
    }

    pub fn frame_buffer_len(&self) -> usize {
        self.core.frame_buffer.front().len()
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.core.set_button(button, pressed);
    }

    pub fn get_cartridge_ram(&self) -> Vec<u8> {
        self.core.memory.get_cartridge_ram().to_vec()
    }

    pub fn load_cartridge_ram(&mut self, data: &[u8]) {
        self.core.memory.load_cartridge_ram(data);
    }

    /// Set camera image data from webcam.
    /// Expects 128x112 pixels as raw 8-bit grayscale (0=black, 255=white).
    pub fn set_camera_image(&mut self, data: &[u8]) {
        self.core.set_camera_image(data);
    }

    /// Check if camera image is ready for capture.
    pub fn is_camera_ready(&self) -> bool {
        self.core.is_camera_ready()
    }

    /// Check if the loaded ROM is a Game Boy Camera cartridge.
    pub fn is_camera(&self) -> bool {
        self.core.is_camera_cartridge()
    }

    /// Update the camera live view buffer if the capture has changed.
    /// Returns true if the buffer was updated.
    pub fn update_camera_live(&mut self) -> bool {
        self.core.update_camera_live()
    }

    /// Pointer to the camera live view RGBA buffer (128x112x4 bytes).
    pub fn camera_live_ptr(&self) -> *const u8 {
        self.core.camera_live_buffer.front().as_ptr()
    }

    /// Length of the camera live view buffer.
    pub fn camera_live_len(&self) -> usize {
        self.core.camera_live_buffer.front().len()
    }

    /// Decode a GB Camera saved photo slot to RGBA pixel data.
    /// Slots 1-30 = saved photos. Returns empty if slot is unoccupied.
    pub fn decode_camera_photo(&self, slot: u8) -> Vec<u8> {
        self.core.decode_camera_photo(slot)
    }

    /// Read a camera hardware register (0x00-0x7F, corresponding to A000-A07F).
    pub fn camera_reg(&self, index: u8) -> u8 {
        self.core.memory.camera_reg(index)
    }

    /// Derive the contrast level (0-15) from the current dither matrix, or -1 if unknown.
    pub fn camera_contrast(&self) -> i32 {
        self.core.memory.camera_contrast()
    }

    /// Get serial output as a string (for test ROM debugging).
    pub fn get_serial_output(&self) -> String {
        self.core.memory.get_serial_output_string()
    }

    /// Clear the serial output buffer.
    pub fn clear_serial_output(&mut self) {
        self.core.memory.clear_serial_output();
    }

    /// Get debug info about the emulator state and log to console.
    pub fn get_debug_info(&self) -> String {
        let info = format!(
            "MBC: {:?}, ROM banks: {}, LCDC: 0x{:02X}, LY: {}",
            self.core.memory.get_mbc_type(),
            self.core.memory.get_rom_bank_count(),
            self.core.memory.read_io_direct(io::LCDC),
            self.core.memory.read_io_direct(io::LY),
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
            self.core.frame_count,
            self.core.total_cycles,
            self.core.instruction_count,
            instructions_this_frame
        );

        log_info!(LogCategory::Cpu, "{}", self.core.cpu.get_debug_state());
        log_info!(LogCategory::Ppu, "{}", self.core.ppu.get_debug_state());
        log_info!(LogCategory::Memory, "{}", self.core.memory.get_io_state());
        log_info!(
            LogCategory::Memory,
            "{}",
            self.core.memory.get_debug_state()
        );

        if !self.core.memory.is_lcd_enabled() {
            log_warn!(LogCategory::General, "LCD is disabled (LCDC bit 7 = 0)");
        }

        log_info!(
            LogCategory::Ppu,
            "buffer non-zero pixels: {}",
            self.core.ppu.count_non_zero_pixels()
        );
    }

    /// Get frame count for debugging.
    pub fn get_frame_count(&self) -> u32 {
        self.core.frame_count
    }

    /// Get total instruction count for debugging.
    pub fn get_instruction_count(&self) -> u64 {
        self.core.instruction_count
    }

    /// Log detailed VRAM tile data for debugging.
    pub fn log_vram_info(&self) {
        let lcdc = self.core.memory.read_io_direct(io::LCDC);
        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x08 != 0 { 0x9C00 } else { 0x9800 };

        log_info!(
            LogCategory::Ppu,
            "VRAM: tile_data={:04X} tile_map={:04X}",
            tile_data_base,
            tile_map_base
        );

        let tile_indices: Vec<String> = (0..16)
            .map(|i| format!("{:02X}", self.core.memory.read(tile_map_base + i)))
            .collect();
        log_info!(LogCategory::Ppu, "Tile indices: {}", tile_indices.join(" "));

        let tile_data: Vec<String> = (0..16)
            .map(|i| format!("{:02X}", self.core.memory.read(0x8000 + i)))
            .collect();
        log_info!(LogCategory::Ppu, "Tile 0 data: {}", tile_data.join(" "));
    }
}

// ── Debug methods ──────────────────────────────────────────────────

#[wasm_bindgen]
impl GameBoy {
    // Execution control

    /// Execute a single CPU instruction, return cycles consumed.
    pub fn step_instruction(&mut self) -> u32 {
        self.core.step_single()
    }

    // CPU state

    pub fn cpu_pc(&self) -> u16 {
        self.core.cpu.get_debug_state().pc
    }

    pub fn cpu_sp(&self) -> u16 {
        self.core.cpu.get_debug_state().sp
    }

    pub fn cpu_a(&self) -> u8 {
        self.core.cpu.get_debug_state().a
    }

    pub fn cpu_f(&self) -> u8 {
        self.core.cpu.get_debug_state().f
    }

    pub fn cpu_bc(&self) -> u16 {
        self.core.cpu.get_debug_state().bc
    }

    pub fn cpu_de(&self) -> u16 {
        self.core.cpu.get_debug_state().de
    }

    pub fn cpu_hl(&self) -> u16 {
        self.core.cpu.get_debug_state().hl
    }

    pub fn cpu_ime(&self) -> bool {
        self.core.cpu.get_debug_state().ime
    }

    pub fn cpu_halted(&self) -> bool {
        self.core.cpu.get_debug_state().halted
    }

    // PPU state

    pub fn ppu_mode(&self) -> u8 {
        self.core.ppu.get_debug_state().mode
    }

    pub fn ppu_line(&self) -> u8 {
        self.core.ppu.get_debug_state().line
    }

    pub fn ppu_cycles(&self) -> u32 {
        self.core.ppu.get_debug_state().cycles
    }

    // Memory access

    pub fn read_byte(&self, addr: u16) -> u8 {
        self.core.memory.read(addr)
    }

    pub fn read_range(&self, addr: u16, len: u16) -> Vec<u8> {
        let mut data = Vec::with_capacity(len as usize);
        for i in 0..len {
            data.push(self.core.memory.read(addr.wrapping_add(i)));
        }
        data
    }

    // IO registers

    pub fn io_lcdc(&self) -> u8 {
        self.core.memory.read_io_direct(io::LCDC)
    }

    pub fn io_stat(&self) -> u8 {
        self.core.memory.read_io_direct(io::STAT)
    }

    pub fn io_scy(&self) -> u8 {
        self.core.memory.read_io_direct(io::SCY)
    }

    pub fn io_scx(&self) -> u8 {
        self.core.memory.read_io_direct(io::SCX)
    }

    pub fn io_ly(&self) -> u8 {
        self.core.memory.read_io_direct(io::LY)
    }

    pub fn io_lyc(&self) -> u8 {
        self.core.memory.read_io_direct(io::LYC)
    }

    pub fn io_bgp(&self) -> u8 {
        self.core.memory.read_io_direct(io::BGP)
    }

    pub fn io_obp0(&self) -> u8 {
        self.core.memory.read_io_direct(io::OBP0)
    }

    pub fn io_obp1(&self) -> u8 {
        self.core.memory.read_io_direct(io::OBP1)
    }

    pub fn io_wy(&self) -> u8 {
        self.core.memory.read_io_direct(io::WY)
    }

    pub fn io_wx(&self) -> u8 {
        self.core.memory.read_io_direct(io::WX)
    }

    pub fn io_ie(&self) -> u8 {
        self.core.memory.read(0xFFFF)
    }

    pub fn io_if(&self) -> u8 {
        self.core.memory.read_io_direct(io::IF)
    }

    pub fn io_div(&self) -> u8 {
        self.core.memory.read_io_direct(io::DIV)
    }

    pub fn io_tima(&self) -> u8 {
        self.core.memory.read_io_direct(io::TIMA)
    }

    pub fn io_tma(&self) -> u8 {
        self.core.memory.read_io_direct(io::TMA)
    }

    pub fn io_tac(&self) -> u8 {
        self.core.memory.read_io_direct(io::TAC)
    }

    pub fn io_joypad(&self) -> u8 {
        self.core.memory.read_io_direct(io::JOYP)
    }
}

impl Default for GameBoy {
    fn default() -> Self {
        Self::new()
    }
}
