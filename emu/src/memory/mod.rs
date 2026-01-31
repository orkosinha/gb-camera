//! Game Boy memory subsystem and cartridge (MBC) emulation.
//!
//! Implements the full 64KB address space including ROM banking (MBC1/MBC3/MBC5),
//! external RAM, VRAM, Work RAM, OAM, and I/O registers. Also handles
//! Game Boy Camera (Pocket Camera) cartridge-specific features including
//! sensor emulation and photo decoding.

mod camera;

use std::fmt;

use crate::log::{LogCategory, RateLimiter};
use crate::{log_info, log_info_limited};

const ROM_BANK_SIZE: usize = 0x4000; // 16KB
const RAM_BANK_SIZE: usize = 0x2000; // 8KB

/// Named constants for Game Boy I/O register offsets (relative to 0xFF00).
pub(crate) mod io {
    pub const JOYP: u8 = 0x00;
    pub const DIV: u8 = 0x04;
    pub const TIMA: u8 = 0x05;
    pub const TMA: u8 = 0x06;
    pub const TAC: u8 = 0x07;
    pub const IF: u8 = 0x0F;
    pub const LCDC: u8 = 0x40;
    pub const STAT: u8 = 0x41;
    pub const SCY: u8 = 0x42;
    pub const SCX: u8 = 0x43;
    pub const LY: u8 = 0x44;
    pub const LYC: u8 = 0x45;
    pub const BGP: u8 = 0x47;
    pub const OBP0: u8 = 0x48;
    pub const OBP1: u8 = 0x49;
    pub const WY: u8 = 0x4A;
    pub const WX: u8 = 0x4B;
}

/// Debug state for Memory inspection.
#[allow(dead_code)]
pub struct MemoryDebugState {
    pub rom_bank: u16,
    pub ram_bank: u8,
    pub ram_enabled: bool,
    pub mbc_type: MbcType,
}

impl fmt::Display for MemoryDebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MBC={:?} ROM_bank={} RAM_bank={} RAM_enabled={}",
            self.mbc_type, self.rom_bank, self.ram_bank, self.ram_enabled
        )
    }
}

/// I/O register state for debugging.
#[allow(dead_code)]
pub struct IoState {
    pub lcdc: u8,
    pub stat: u8,
    pub ly: u8,
    pub ie: u8,
    pub if_reg: u8,
    pub scy: u8,
    pub scx: u8,
    pub bgp: u8,
}

impl fmt::Display for IoState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LCDC={:02X}(LCD={} BG={} OBJ={} WIN={}) STAT={:02X} LY={} IE={:02X} IF={:02X} SCY={} SCX={} BGP={:02X}",
            self.lcdc,
            if self.lcdc & 0x80 != 0 { "ON" } else { "off" },
            if self.lcdc & 0x01 != 0 { "ON" } else { "off" },
            if self.lcdc & 0x02 != 0 { "ON" } else { "off" },
            if self.lcdc & 0x20 != 0 { "ON" } else { "off" },
            self.stat,
            self.ly,
            self.ie,
            self.if_reg,
            self.scy,
            self.scx,
            self.bgp
        )
    }
}

/// Cartridge/MBC type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MbcType {
    None,         // No MBC (32KB ROM only)
    Mbc1,         // MBC1
    Mbc3,         // MBC3 (with RTC support)
    Mbc5,         // MBC5
    PocketCamera, // Game Boy Camera (0xFC)
}

pub struct Memory {
    // Cartridge
    rom: Vec<u8>,
    cartridge_ram: Vec<u8>,
    rom_bank: u16, // Changed to u16 for MBC5 support (up to 512 banks)
    ram_bank: u8,
    ram_enabled: bool,
    mbc_type: MbcType,

    // MBC1 specific
    mbc1_mode: bool, // false = ROM mode, true = RAM mode

    // Internal memory
    vram: [u8; 0x2000], // 0x8000-0x9FFF
    wram: [u8; 0x2000], // 0xC000-0xDFFF
    oam: [u8; 0xA0],    // 0xFE00-0xFE9F
    io: [u8; 0x80],     // 0xFF00-0xFF7F
    hram: [u8; 0x7F],   // 0xFF80-0xFFFE
    ie: u8,             // 0xFFFF - Interrupt Enable

    // Serial output buffer (for test ROM debugging)
    serial_output: Vec<u8>,

    // Camera registers (active when ram_bank >= 0x10 on Pocket Camera)
    camera_regs: [u8; 0x80],

    // Camera image buffer: 128x112 pixels, raw 8-bit grayscale (0=black, 255=white)
    // This is set from JavaScript webcam and processed by sensor emulation on capture
    camera_image: Box<[u8; 128 * 112]>,
    camera_image_ready: bool,
    camera_capture_dirty: bool,
}

impl Memory {
    pub fn new() -> Self {
        let mut mem = Memory {
            rom: Vec::new(),
            cartridge_ram: vec![0; 128 * 1024], // 128KB for camera
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            mbc_type: MbcType::None,
            mbc1_mode: false,
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            serial_output: Vec::new(),
            camera_regs: [0; 0x80],
            camera_image: Box::new([0; 128 * 112]),
            camera_image_ready: false,
            camera_capture_dirty: false,
        };
        mem.init_io_defaults();
        mem
    }

    fn init_io_defaults(&mut self) {
        // Initial I/O register values after boot
        self.io[0x00] = 0xCF; // P1/JOYP
        self.io[0x01] = 0x00; // SB
        self.io[0x02] = 0x7E; // SC
        self.io[0x04] = 0xAB; // DIV
        self.io[0x05] = 0x00; // TIMA
        self.io[0x06] = 0x00; // TMA
        self.io[0x07] = 0xF8; // TAC
        self.io[0x0F] = 0xE1; // IF
        self.io[0x40] = 0x91; // LCDC
        self.io[0x41] = 0x85; // STAT
        self.io[0x42] = 0x00; // SCY
        self.io[0x43] = 0x00; // SCX
        self.io[0x44] = 0x00; // LY
        self.io[0x45] = 0x00; // LYC
        self.io[0x47] = 0xFC; // BGP
        self.io[0x48] = 0xFF; // OBP0
        self.io[0x49] = 0xFF; // OBP1
        self.io[0x4A] = 0x00; // WY
        self.io[0x4B] = 0x00; // WX
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < 0x150 {
            return Err("ROM too small");
        }

        // Detect MBC type from cartridge header (0x0147)
        let cart_type = data[0x0147];
        self.mbc_type = match cart_type {
            0x00 => MbcType::None,
            0x01..=0x03 => MbcType::Mbc1,
            0x0F..=0x13 => MbcType::Mbc3,
            0x19..=0x1E => MbcType::Mbc5,
            0xFC => MbcType::PocketCamera,
            _ => {
                // Default to MBC5 for unknown types (most compatible)
                MbcType::Mbc5
            }
        };

        // Determine RAM size from header (0x0149)
        let ram_size = match data[0x0149] {
            0x00 => 0,
            0x01 => 2 * 1024,   // 2KB (unofficial)
            0x02 => 8 * 1024,   // 8KB
            0x03 => 32 * 1024,  // 32KB (4 banks)
            0x04 => 128 * 1024, // 128KB (16 banks)
            0x05 => 64 * 1024,  // 64KB (8 banks)
            _ => 128 * 1024,    // Default to max
        };

        // Game Boy Camera always has 128KB RAM
        let ram_size = if self.mbc_type == MbcType::PocketCamera {
            128 * 1024
        } else if ram_size == 0 {
            // Some games don't report RAM size correctly
            8 * 1024
        } else {
            ram_size
        };

        self.cartridge_ram = vec![0; ram_size];
        self.rom = data.to_vec();
        self.rom_bank = 1;
        self.ram_bank = 0;
        self.ram_enabled = false;
        self.mbc1_mode = false;

        Ok(())
    }

    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM Bank 0 (fixed)
            0x0000..=0x3FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF),

            // ROM Bank 1-N (switchable)
            0x4000..=0x7FFF => {
                let bank = self.rom_bank.max(1) as usize; // Bank 0 maps to bank 1
                let offset = bank * ROM_BANK_SIZE + (addr as usize - 0x4000);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }

            // Video RAM
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],

            // External RAM / Camera registers
            0xA000..=0xBFFF => {
                // Game Boy Camera: bank 0x10+ maps to camera registers
                // Camera registers are accessible regardless of ram_enabled state
                if self.mbc_type == MbcType::PocketCamera && self.ram_bank >= 0x10 {
                    let reg_addr = (addr - 0xA000) as usize;
                    if reg_addr < 0x80 {
                        let value = self.camera_regs[reg_addr];
                        // Log reads of capture status register
                        if reg_addr == 0 {
                            static A000_READ_LIMITER: RateLimiter = RateLimiter::new(50);
                            log_info_limited!(
                                LogCategory::Camera,
                                &A000_READ_LIMITER,
                                "Read A000 (capture status): 0x{:02X} (busy={})",
                                value,
                                (value & 0x01) != 0
                            );
                        }
                        return value;
                    }
                    // A080-AFFF: Camera sensor output / captured tile data
                    // The captured image is available here after capture completes
                    // This maps to the same data we store in SRAM at offset 0x0100
                    let tile_offset = reg_addr - 0x80;
                    if tile_offset < 0x0E00 {
                        // Map to the captured image data in SRAM
                        let sram_addr = 0x0100 + tile_offset;
                        let value = self.cartridge_ram.get(sram_addr).copied().unwrap_or(0x00);
                        static TILE_READ_LIMITER: RateLimiter = RateLimiter::new(20);
                        log_info_limited!(
                            LogCategory::Camera,
                            &TILE_READ_LIMITER,
                            "Read camera tile data A{:03X} -> SRAM[{:04X}] = {:02X}",
                            reg_addr,
                            sram_addr,
                            value
                        );
                        return value;
                    }
                    return 0x00;
                }

                if !self.ram_enabled {
                    // For Pocket Camera: allow full SRAM access even with RAM disabled
                    // The Game Boy Camera doesn't require RAM enable for SRAM operations
                    if self.mbc_type == MbcType::PocketCamera {
                        let offset =
                            (self.ram_bank as usize) * RAM_BANK_SIZE + (addr - 0xA000) as usize;
                        return self.cartridge_ram.get(offset).copied().unwrap_or(0x00);
                    }
                    return 0xFF;
                }

                let offset = (self.ram_bank as usize) * RAM_BANK_SIZE + (addr - 0xA000) as usize;
                let value = self.cartridge_ram.get(offset).copied().unwrap_or(0xFF);

                // Log reads from camera image area for debugging
                // Expanded range to catch all captured tile data reads (A100-AEFF = 0x0E00 bytes)
                if self.mbc_type == MbcType::PocketCamera && (0xA100..0xAF00).contains(&addr) {
                    static SRAM_READ_LIMITER: RateLimiter = RateLimiter::new(50);
                    log_info_limited!(
                        LogCategory::Camera,
                        &SRAM_READ_LIMITER,
                        "SRAM read: {:04X} bank={} offset={:04X} -> {:02X}",
                        addr,
                        self.ram_bank,
                        offset,
                        value
                    );
                }

                value
            }

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],

            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],

            // OAM
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],

            // Unusable
            0xFEA0..=0xFEFF => 0xFF,

            // I/O Registers
            0xFF00..=0xFF7F => self.read_io(addr),

            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],

            // Interrupt Enable
            0xFFFF => self.ie,
        }
    }

    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            // RAM Enable (0x0000-0x1FFF)
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }

            // ROM Bank select (0x2000-0x3FFF)
            0x2000..=0x3FFF => {
                match self.mbc_type {
                    MbcType::None => {}
                    MbcType::Mbc1 => {
                        // MBC1: 5-bit bank number (bits 0-4)
                        let bank = value & 0x1F;
                        self.rom_bank =
                            (self.rom_bank & 0x60) | (if bank == 0 { 1 } else { bank }) as u16;
                    }
                    MbcType::Mbc3 | MbcType::PocketCamera => {
                        // MBC3/Camera: 7-bit bank number
                        let bank = value & 0x7F;
                        self.rom_bank = if bank == 0 { 1 } else { bank as u16 };
                    }
                    MbcType::Mbc5 => {
                        // MBC5: Low 8 bits of bank number
                        if addr < 0x3000 {
                            self.rom_bank = (self.rom_bank & 0x100) | (value as u16);
                        } else {
                            // High bit of bank number (0x3000-0x3FFF)
                            self.rom_bank = (self.rom_bank & 0xFF) | ((value as u16 & 1) << 8);
                        }
                    }
                }
            }

            // RAM Bank select / Upper ROM bank bits (0x4000-0x5FFF)
            0x4000..=0x5FFF => {
                match self.mbc_type {
                    MbcType::None => {}
                    MbcType::Mbc1 => {
                        if self.mbc1_mode {
                            // RAM banking mode
                            self.ram_bank = value & 0x03;
                        } else {
                            // ROM banking mode - upper 2 bits
                            self.rom_bank = (self.rom_bank & 0x1F) | ((value as u16 & 0x03) << 5);
                        }
                    }
                    MbcType::Mbc3 | MbcType::Mbc5 => {
                        self.ram_bank = value & 0x0F;
                    }
                    MbcType::PocketCamera => {
                        // Camera: bank 0x00-0x0F = RAM, 0x10+ = camera registers
                        let new_bank = value & 0x1F;
                        // Always log bank switches for debugging
                        log_info!(
                            LogCategory::Camera,
                            "RAM bank: {} -> {} (mode={})",
                            self.ram_bank,
                            new_bank,
                            if new_bank >= 0x10 {
                                "CAMERA_REGS"
                            } else {
                                "SRAM"
                            }
                        );
                        self.ram_bank = new_bank;
                    }
                }
            }

            // Banking mode select (0x6000-0x7FFF)
            0x6000..=0x7FFF => {
                if self.mbc_type == MbcType::Mbc1 {
                    self.mbc1_mode = (value & 0x01) != 0;
                }
            }

            // Video RAM
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = value,

            // External RAM / Camera registers
            0xA000..=0xBFFF => {
                // Game Boy Camera: bank 0x10+ = camera registers
                // Camera registers are accessible regardless of ram_enabled state
                if self.mbc_type == MbcType::PocketCamera && self.ram_bank >= 0x10 {
                    let reg_addr = (addr - 0xA000) as usize;
                    if reg_addr < 0x80 {
                        // Log all camera register writes for debugging
                        if reg_addr == 0 {
                            log_info!(
                                LogCategory::Camera,
                                "Write A000: 0x{:02X} (capture={}, N={}, VH={})",
                                value,
                                (value & 0x01) != 0,
                                (value >> 1) & 0x01,
                                (value >> 2) & 0x03
                            );
                        } else if reg_addr <= 0x35 {
                            // Log other sensor registers (A001-A035)
                            static REG_LIMITER: RateLimiter = RateLimiter::new(100);
                            log_info_limited!(
                                LogCategory::Camera,
                                &REG_LIMITER,
                                "Write A0{:02X}: 0x{:02X}",
                                reg_addr,
                                value
                            );
                        }

                        self.camera_regs[reg_addr] = value;
                        // Register 0 bit 0: 1 = start capture, 0 = capture complete
                        if reg_addr == 0 && (value & 0x01) != 0 {
                            // Extract capture parameters
                            let invert = (value & 0x02) != 0; // N flag: bit 1 = invert/negative
                            log_info!(
                                LogCategory::Camera,
                                "Capture triggered! image_ready={}, invert={}, Processing...",
                                self.camera_image_ready,
                                invert
                            );

                            // Capture triggered - convert camera_image to tiles in SRAM
                            self.process_camera_capture(invert);
                            self.camera_capture_dirty = true;
                            // Clear capture bit to indicate completion
                            self.camera_regs[0] &= !0x01;

                            log_info!(
                                LogCategory::Camera,
                                "Capture complete, A000 now=0x{:02X}",
                                self.camera_regs[0]
                            );
                        }
                    }
                    return;
                }

                // For Pocket Camera: allow SRAM access even with RAM disabled
                if !self.ram_enabled && self.mbc_type != MbcType::PocketCamera {
                    return;
                }

                let offset = (self.ram_bank as usize) * RAM_BANK_SIZE + (addr - 0xA000) as usize;
                if offset < self.cartridge_ram.len() {
                    self.cartridge_ram[offset] = value;
                }
            }

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = value,

            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = value,

            // OAM
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,

            // Unusable
            0xFEA0..=0xFEFF => {}

            // I/O Registers
            0xFF00..=0xFF7F => self.write_io(addr, value),

            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,

            // Interrupt Enable
            0xFFFF => self.ie = value,
        }
    }

    #[inline]
    fn read_io(&self, addr: u16) -> u8 {
        let offset = (addr - 0xFF00) as usize;
        match offset {
            0x00 => self.io[0x00] | 0xC0, // JOYP: upper bits always 1
            _ => self.io[offset],
        }
    }

    #[inline]
    fn write_io(&mut self, addr: u16, value: u8) {
        let offset = (addr - 0xFF00) as usize;
        match offset {
            0x02 => {
                // SC (Serial Control): When bit 7 is set, transfer is requested
                self.io[0x02] = value;
                if value & 0x80 != 0 {
                    // Transfer the byte in SB to serial output
                    let sb = self.io[0x01];
                    self.serial_output.push(sb);
                    // Clear transfer bit (transfer complete)
                    self.io[0x02] &= 0x7F;
                }
            }
            0x04 => self.io[0x04] = 0, // DIV: writing any value resets to 0
            0x44 => {}                 // LY: read-only
            0x46 => self.dma_transfer(value),
            _ => self.io[offset] = value,
        }
    }

    fn dma_transfer(&mut self, value: u8) {
        let source = (value as u16) << 8;
        for i in 0..0xA0 {
            self.oam[i] = self.read(source + i as u16);
        }
    }

    // I/O register accessors for other components
    #[inline]
    pub fn read_io_direct(&self, offset: u8) -> u8 {
        self.io[offset as usize]
    }

    #[inline]
    pub fn write_io_direct(&mut self, offset: u8, value: u8) {
        self.io[offset as usize] = value;
    }

    #[inline]
    pub fn get_ie(&self) -> u8 {
        self.ie
    }

    #[inline]
    pub fn get_oam(&self) -> &[u8] {
        &self.oam
    }

    pub fn get_cartridge_ram(&self) -> &[u8] {
        &self.cartridge_ram
    }

    pub fn load_cartridge_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.cartridge_ram.len());
        self.cartridge_ram[..len].copy_from_slice(&data[..len]);
    }

    /// Get serial output as a string (for test ROM debugging).
    #[allow(dead_code)]
    pub fn get_serial_output_string(&self) -> String {
        String::from_utf8_lossy(&self.serial_output).to_string()
    }

    /// Clear the serial output buffer.
    #[allow(dead_code)]
    pub fn clear_serial_output(&mut self) {
        self.serial_output.clear();
    }

    /// Get the detected MBC type.
    pub fn get_mbc_type(&self) -> MbcType {
        self.mbc_type
    }

    /// Get the number of ROM banks.
    #[allow(dead_code)]
    pub fn get_rom_bank_count(&self) -> usize {
        self.rom.len() / ROM_BANK_SIZE
    }

    /// Get current memory state for debugging.
    #[allow(dead_code)]
    pub fn get_debug_state(&self) -> MemoryDebugState {
        MemoryDebugState {
            rom_bank: self.rom_bank,
            ram_bank: self.ram_bank,
            ram_enabled: self.ram_enabled,
            mbc_type: self.mbc_type,
        }
    }

    /// Get current I/O register state for debugging.
    #[allow(dead_code)]
    pub fn get_io_state(&self) -> IoState {
        IoState {
            lcdc: self.io[0x40],
            stat: self.io[0x41],
            ly: self.io[0x44],
            ie: self.ie,
            if_reg: self.io[0x0F],
            scy: self.io[0x42],
            scx: self.io[0x43],
            bgp: self.io[0x47],
        }
    }

    /// Check if LCD is enabled.
    #[allow(dead_code)]
    pub fn is_lcd_enabled(&self) -> bool {
        self.io[0x40] & 0x80 != 0
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wram_read_write() {
        let mut mem = Memory::new();
        mem.write(0xC000, 0x42);
        assert_eq!(mem.read(0xC000), 0x42);

        mem.write(0xDFFF, 0xFF);
        assert_eq!(mem.read(0xDFFF), 0xFF);
    }

    #[test]
    fn test_echo_ram() {
        let mut mem = Memory::new();
        mem.write(0xC000, 0x42);
        // Echo RAM mirrors 0xC000-0xDDFF at 0xE000-0xFDFF
        assert_eq!(mem.read(0xE000), 0x42);

        mem.write(0xE100, 0x55);
        assert_eq!(mem.read(0xC100), 0x55);
    }

    #[test]
    fn test_hram() {
        let mut mem = Memory::new();
        mem.write(0xFF80, 0x12);
        assert_eq!(mem.read(0xFF80), 0x12);

        mem.write(0xFFFE, 0x34);
        assert_eq!(mem.read(0xFFFE), 0x34);
    }

    #[test]
    fn test_ie_register() {
        let mut mem = Memory::new();
        mem.write(0xFFFF, 0x1F);
        assert_eq!(mem.read(0xFFFF), 0x1F);
        assert_eq!(mem.get_ie(), 0x1F);
    }

    #[test]
    fn test_vram() {
        let mut mem = Memory::new();
        mem.write(0x8000, 0xAA);
        assert_eq!(mem.read(0x8000), 0xAA);

        mem.write(0x9FFF, 0xBB);
        assert_eq!(mem.read(0x9FFF), 0xBB);
    }

    #[test]
    fn test_oam() {
        let mut mem = Memory::new();
        mem.write(0xFE00, 0x10);
        assert_eq!(mem.read(0xFE00), 0x10);

        mem.write(0xFE9F, 0x20);
        assert_eq!(mem.read(0xFE9F), 0x20);
    }

    #[test]
    fn test_unusable_region() {
        let mem = Memory::new();
        // Unusable region should return 0xFF
        assert_eq!(mem.read(0xFEA0), 0xFF);
        assert_eq!(mem.read(0xFEFF), 0xFF);
    }

    #[test]
    fn test_rom_bank_switching() {
        let mut mem = Memory::new();

        // Create a test ROM with multiple banks
        let mut rom = vec![0u8; 0x8000]; // 32KB = 2 banks
        rom[0x0000] = 0x11; // Bank 0
        rom[0x4000] = 0x22; // Bank 1 at 0x4000

        mem.load_rom(&rom).unwrap();

        // Bank 0 is always at 0x0000-0x3FFF
        assert_eq!(mem.read(0x0000), 0x11);

        // Bank 1 is default at 0x4000-0x7FFF
        assert_eq!(mem.read(0x4000), 0x22);
    }

    #[test]
    fn test_rom_bank_select() {
        let mut mem = Memory::new();

        // Create a test ROM with 4 banks
        let mut rom = vec![0u8; 0x10000]; // 64KB = 4 banks
        rom[0x0147] = 0x01; // MBC1 cartridge type
        rom[0x4000] = 0x11; // Bank 1
        rom[0x8000] = 0x22; // Bank 2
        rom[0xC000] = 0x33; // Bank 3

        mem.load_rom(&rom).unwrap();

        // Select bank 2
        mem.write(0x2000, 0x02);
        assert_eq!(mem.read(0x4000), 0x22);

        // Select bank 3
        mem.write(0x2000, 0x03);
        assert_eq!(mem.read(0x4000), 0x33);

        // Bank 0 written as 1 (bank 0 not selectable for switchable area)
        mem.write(0x2000, 0x00);
        assert_eq!(mem.read(0x4000), 0x11);
    }

    #[test]
    fn test_external_ram_enable() {
        let mut mem = Memory::new();

        // RAM disabled by default, should return 0xFF
        assert_eq!(mem.read(0xA000), 0xFF);

        // Enable RAM
        mem.write(0x0000, 0x0A);
        mem.write(0xA000, 0x42);
        assert_eq!(mem.read(0xA000), 0x42);

        // Disable RAM
        mem.write(0x0000, 0x00);
        assert_eq!(mem.read(0xA000), 0xFF);
    }

    #[test]
    fn test_div_reset() {
        let mut mem = Memory::new();
        mem.write_io_direct(0x04, 0xFF);

        // Writing any value to DIV resets it to 0
        mem.write(0xFF04, 0x42);
        assert_eq!(mem.read(0xFF04), 0x00);
    }

    #[test]
    fn test_ly_readonly() {
        let mut mem = Memory::new();
        mem.write_io_direct(0x44, 0x50);

        // Writing to LY should not change it
        mem.write(0xFF44, 0x99);
        assert_eq!(mem.read(0xFF44), 0x50);
    }

    #[test]
    fn test_dma_transfer() {
        let mut mem = Memory::new();

        // Set up source data in WRAM
        for i in 0..0xA0 {
            mem.write(0xC000 + i as u16, i as u8);
        }

        // Trigger DMA from 0xC000
        mem.write(0xFF46, 0xC0);

        // Check OAM was populated
        for i in 0..0xA0 {
            assert_eq!(mem.read(0xFE00 + i as u16), i as u8);
        }
    }

    #[test]
    fn test_load_rom_too_small() {
        let mut mem = Memory::new();
        let small_rom = vec![0u8; 0x100]; // Too small
        assert!(mem.load_rom(&small_rom).is_err());
    }

    #[test]
    fn test_cartridge_ram_persistence() {
        let mut mem = Memory::new();

        // Enable RAM and write
        mem.write(0x0000, 0x0A);
        mem.write(0xA000, 0x42);
        mem.write(0xA001, 0x43);

        // Get RAM
        let ram = mem.get_cartridge_ram();
        assert_eq!(ram[0], 0x42);
        assert_eq!(ram[1], 0x43);

        // Load RAM into new memory
        let mut mem2 = Memory::new();
        mem2.load_cartridge_ram(&ram);
        mem2.write(0x0000, 0x0A); // Enable RAM
        assert_eq!(mem2.read(0xA000), 0x42);
        assert_eq!(mem2.read(0xA001), 0x43);
    }
}
