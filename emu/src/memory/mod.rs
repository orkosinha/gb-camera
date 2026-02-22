//! Game Boy memory subsystem.
//!
//! Implements the full 64KB address space. Cartridge (ROM/RAM/MBC) is owned
//! by a `Box<dyn Cartridge>` — see `memory/cartridge/` for implementations.
//! GBC-specific registers (HDMA, VBK, SVBK, palette RAM) are gated behind
//! `cgb.mode` so a DMG ROM cannot accidentally trigger GBC behaviour.

pub(crate) mod camera;
mod cgb;
pub(crate) mod rtc;
pub mod cartridge;

use std::fmt;

use cgb::Cgb;

pub use cartridge::MbcType;
use cartridge::{Cartridge, make_cartridge, ram_size_from_header};

/// Named constants for Game Boy I/O register offsets (relative to 0xFF00).
#[allow(dead_code)] // constants used selectively across wasm/ios/ppu/cpu modules
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
    // GBC registers
    pub const KEY1: u8 = 0x4D; // speed switch
    pub const VBK: u8 = 0x4F;  // VRAM bank
    pub const HDMA1: u8 = 0x51; // DMA source high
    pub const HDMA2: u8 = 0x52; // DMA source low
    pub const HDMA3: u8 = 0x53; // DMA dest high
    pub const HDMA4: u8 = 0x54; // DMA dest low
    pub const HDMA5: u8 = 0x55; // DMA control/trigger
    pub const RP: u8 = 0x56;    // Infrared (stub)
    pub const BCPS: u8 = 0x68;  // BG palette index
    pub const BCPD: u8 = 0x69;  // BG palette data
    pub const OCPS: u8 = 0x6A;  // OBJ palette index
    pub const OCPD: u8 = 0x6B;  // OBJ palette data
    pub const SVBK: u8 = 0x70;  // WRAM bank
}

/// Debug state for Memory inspection.
#[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: log_frame_debug
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
#[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: load_rom, log_frame_debug
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

pub struct Memory {
    // Cartridge: owns ROM, RAM, and all MBC banking state
    cartridge: Box<dyn Cartridge>,

    // Internal memory
    vram: [[u8; 0x2000]; 2], // 0x8000-0x9FFF; bank 0 = tiles, bank 1 = GBC tile attrs
    wram: [[u8; 0x1000]; 8], // bank 0 fixed (0xC000-0xCFFF), banks 1-7 switchable (0xD000-0xDFFF)
    oam: [u8; 0xA0],         // 0xFE00-0xFE9F
    io: [u8; 0x80],          // 0xFF00-0xFF7F
    hram: [u8; 0x7F],        // 0xFF80-0xFFFE
    ie: u8,                  // 0xFFFF - Interrupt Enable

    // GBC-specific state (palette RAM, banking control, double-speed, HDMA)
    cgb: Cgb,

    // Serial output buffer (for test ROM debugging)
    serial_output: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        // Default cartridge: NoMbc with empty ROM
        let cartridge: Box<dyn Cartridge> =
            Box::new(cartridge::NoMbc::new(vec![]));
        let mut mem = Memory {
            cartridge,
            vram: [[0; 0x2000]; 2],
            wram: [[0; 0x1000]; 8],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            cgb: Cgb::new(),
            serial_output: Vec::new(),
        };
        mem.init_io_defaults();
        mem
    }

    fn init_io_defaults(&mut self) {
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

    pub fn load_rom(&mut self, data: &[u8], cgb_mode: bool) -> Result<(), &'static str> {
        if data.len() < 0x150 {
            return Err("ROM too small");
        }

        let cart_type = data[0x0147];
        let ram_size = if cart_type == 0xFC {
            128 * 1024 // Game Boy Camera always has 128KB RAM
        } else {
            ram_size_from_header(data[0x0149])
        };

        // Reset hardware state (power cycle)
        self.vram = [[0; 0x2000]; 2];
        self.wram = [[0; 0x1000]; 8];
        self.oam = [0; 0xA0];
        self.io = [0; 0x80];
        self.hram = [0; 0x7F];
        self.ie = 0;
        self.cgb = Cgb::new();
        self.cgb.mode = cgb_mode;
        self.init_io_defaults();

        self.cartridge = make_cartridge(data.to_vec(), cart_type, ram_size);

        Ok(())
    }

    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM (cartridge owns bank switching)
            0x0000..=0x7FFF => self.cartridge.read_rom(addr),

            // Video RAM (bank selected by VBK; DMG always uses bank 0)
            0x8000..=0x9FFF => {
                let bank = if self.cgb.mode { self.cgb.vram_bank } else { 0 };
                self.vram[bank][(addr - 0x8000) as usize]
            }

            // External RAM / Camera registers (cartridge handles all logic)
            0xA000..=0xBFFF => self.cartridge.read_ram(addr),

            // Work RAM — bank 0 fixed, banks 1-7 switchable (DMG always uses bank 1)
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize],
            0xD000..=0xDFFF => {
                let bank = if self.cgb.mode { self.cgb.wram_bank } else { 1 };
                self.wram[bank][(addr - 0xD000) as usize]
            }

            // Echo RAM mirrors 0xC000-0xDDFF
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize],
            0xF000..=0xFDFF => {
                let bank = if self.cgb.mode { self.cgb.wram_bank } else { 1 };
                self.wram[bank][(addr - 0xF000) as usize]
            }

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
            // MBC register writes (RAM enable, bank select, mode)
            0x0000..=0x7FFF => self.cartridge.write_rom(addr, value),

            // Video RAM (DMG always uses bank 0)
            0x8000..=0x9FFF => {
                let bank = if self.cgb.mode { self.cgb.vram_bank } else { 0 };
                self.vram[bank][(addr - 0x8000) as usize] = value;
            }

            // External RAM / Camera registers
            0xA000..=0xBFFF => self.cartridge.write_ram(addr, value),

            // Work RAM — bank 0 fixed, banks 1-7 switchable (DMG always uses bank 1)
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize] = value,
            0xD000..=0xDFFF => {
                let bank = if self.cgb.mode { self.cgb.wram_bank } else { 1 };
                self.wram[bank][(addr - 0xD000) as usize] = value;
            }

            // Echo RAM mirrors 0xC000-0xDDFF
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize] = value,
            0xF000..=0xFDFF => {
                let bank = if self.cgb.mode { self.cgb.wram_bank } else { 1 };
                self.wram[bank][(addr - 0xF000) as usize] = value;
            }

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
            // 0xFF00 (joypad) is intercepted by MemoryBus before reaching here
            // 0xFF04-0xFF07 (timer) are intercepted by MemoryBus before reaching here

            // GBC-only registers — return 0xFF in DMG mode (open bus)
            0x4D => {
                if self.cgb.mode {
                    (self.cgb.double_speed as u8) << 7 | 0x7E | self.cgb.speed_armed as u8
                } else {
                    0xFF
                }
            }
            0x4F => {
                if self.cgb.mode {
                    self.cgb.vram_bank as u8 | 0xFE
                } else {
                    0xFF
                }
            }
            0x68 => {
                if self.cgb.mode {
                    self.cgb.bcps | 0x40
                } else {
                    0xFF
                }
            }
            0x69 => {
                if self.cgb.mode {
                    self.cgb.bg_palette_ram[(self.cgb.bcps & 0x3F) as usize]
                } else {
                    0xFF
                }
            }
            0x6A => {
                if self.cgb.mode {
                    self.cgb.ocps | 0x40
                } else {
                    0xFF
                }
            }
            0x6B => {
                if self.cgb.mode {
                    self.cgb.obj_palette_ram[(self.cgb.ocps & 0x3F) as usize]
                } else {
                    0xFF
                }
            }
            0x56 => 0xFF, // RP: infrared stub — open bus in both modes
            0x70 => {
                if self.cgb.mode {
                    self.cgb.wram_bank as u8 | 0xF8
                } else {
                    0xFF
                }
            }
            _ => self.io[offset],
        }
    }

    #[inline]
    fn write_io(&mut self, addr: u16, value: u8) {
        let offset = (addr - 0xFF00) as usize;
        match offset {
            // 0xFF00 (joypad) is intercepted by MemoryBus
            // 0xFF04-0xFF07 (timer) are intercepted by MemoryBus

            0x02 => {
                // SC: when bit 7 set, transfer SB to serial output
                self.io[0x02] = value;
                if value & 0x80 != 0 {
                    let sb = self.io[0x01];
                    self.serial_output.push(sb);
                    self.io[0x02] &= 0x7F;
                }
            }
            0x04 => self.io[0x04] = 0, // DIV: any write resets to 0
            0x44 => {}                 // LY: read-only
            0x46 => self.dma_transfer(value),

            // GBC-only registers — silently ignored in DMG mode
            0x4D => {
                if self.cgb.mode {
                    self.cgb.speed_armed = value & 1 != 0;
                }
            }
            0x4F => {
                if self.cgb.mode {
                    self.cgb.vram_bank = (value & 1) as usize;
                }
            }
            0x51 => {
                if self.cgb.mode {
                    self.io[0x51] = value;
                }
            }
            0x52 => {
                if self.cgb.mode {
                    self.io[0x52] = value;
                }
            }
            0x53 => {
                if self.cgb.mode {
                    self.io[0x53] = value;
                }
            }
            0x54 => {
                if self.cgb.mode {
                    self.io[0x54] = value;
                }
            }
            0x55 => {
                if self.cgb.mode {
                    let source =
                        ((self.io[0x51] as u16) << 8 | self.io[0x52] as u16) & 0xFFF0;
                    let dest = 0x8000u16
                        | (((self.io[0x53] as u16) << 8 | self.io[0x54] as u16) & 0x1FF0);
                    self.cgb.hdma_source = source;
                    self.cgb.hdma_dest = dest;
                    if value & 0x80 == 0 {
                        let blocks = (value & 0x7F) as u16 + 1;
                        let total_bytes = blocks * 16;
                        for i in 0..total_bytes {
                            let src_byte = self.read(self.cgb.hdma_source + i);
                            let dest_vram = (self.cgb.hdma_dest & 0x1FFF) + i;
                            self.vram[self.cgb.vram_bank][dest_vram as usize] = src_byte;
                        }
                        self.cgb.hdma_active = false;
                        self.io[0x55] = 0xFF;
                    } else {
                        self.cgb.hdma_len = value & 0x7F;
                        self.cgb.hdma_active = true;
                        self.cgb.hdma_hblank = true;
                        self.io[0x55] = value & 0x7F;
                    }
                }
            }
            0x56 => {} // RP: infrared (stub, ignore)
            0x68 => {
                if self.cgb.mode {
                    self.cgb.bcps = value;
                }
            }
            0x69 => {
                if self.cgb.mode {
                    self.cgb.bg_palette_ram[(self.cgb.bcps & 0x3F) as usize] = value;
                    if self.cgb.bcps & 0x80 != 0 {
                        self.cgb.bcps =
                            (self.cgb.bcps & 0x80) | ((self.cgb.bcps + 1) & 0x3F);
                    }
                }
            }
            0x6A => {
                if self.cgb.mode {
                    self.cgb.ocps = value;
                }
            }
            0x6B => {
                if self.cgb.mode {
                    self.cgb.obj_palette_ram[(self.cgb.ocps & 0x3F) as usize] = value;
                    if self.cgb.ocps & 0x80 != 0 {
                        self.cgb.ocps =
                            (self.cgb.ocps & 0x80) | ((self.cgb.ocps + 1) & 0x3F);
                    }
                }
            }
            0x70 => {
                if self.cgb.mode {
                    self.cgb.wram_bank = ((value & 7) as usize).max(1);
                }
            }
            _ => self.io[offset] = value,
        }
    }

    fn dma_transfer(&mut self, value: u8) {
        let source = (value as u16) << 8;
        for i in 0..0xA0 {
            self.oam[i] = self.read(source + i as u16);
        }
    }

    // ── I/O register accessors for other components ──────────────────────────

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
        self.cartridge.ram_data()
    }

    pub fn load_cartridge_ram(&mut self, data: &[u8]) {
        self.cartridge.load_ram(data);
    }

    /// Read a camera hardware register directly (index 0x00-0x7F).
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: camera_reg
    pub fn camera_reg(&self, index: u8) -> u8 {
        self.cartridge
            .as_camera()
            .map(|c| c.reg(index))
            .unwrap_or(0xFF)
    }

    /// Set or clear the exposure override for the camera sensor.
    #[cfg_attr(not(feature = "ios"), allow(dead_code))] // ios: gb_set_camera_exposure
    pub fn set_camera_exposure_override(&mut self, value: Option<u16>) {
        if let Some(cam) = self.cartridge.as_camera_mut() {
            cam.set_exposure_override(value);
        }
    }

    /// Get serial output as a string (for test ROM debugging).
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: get_serial_output
    pub fn get_serial_output_string(&self) -> String {
        String::from_utf8_lossy(&self.serial_output).to_string()
    }

    /// Clear the serial output buffer.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: clear_serial_output
    pub fn clear_serial_output(&mut self) {
        self.serial_output.clear();
    }

    /// Advance the RTC (delegated to cartridge; no-op for non-MBC3).
    pub fn tick_rtc(&mut self) {
        self.cartridge.tick_rtc();
    }

    /// Get the detected MBC type.
    pub fn get_mbc_type(&self) -> MbcType {
        self.cartridge.mbc_type()
    }

    /// Get the number of ROM banks.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: load_rom
    pub fn get_rom_bank_count(&self) -> usize {
        self.cartridge.rom_bank_count()
    }

    /// Get current memory state for debugging.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: log_frame_debug
    pub fn get_debug_state(&self) -> MemoryDebugState {
        MemoryDebugState {
            rom_bank: self.cartridge.current_rom_bank(),
            ram_bank: self.cartridge.current_ram_bank(),
            ram_enabled: self.cartridge.is_ram_enabled(),
            mbc_type: self.cartridge.mbc_type(),
        }
    }

    /// Get current I/O register state for debugging.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: load_rom, log_frame_debug
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
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: log_frame_debug
    pub fn is_lcd_enabled(&self) -> bool {
        self.io[0x40] & 0x80 != 0
    }

    // ── GBC accessors ────────────────────────────────────────────────────────

    /// Check if GBC mode is active for this ROM session.
    #[allow(dead_code)] // used by wasm/ios features and tests
    pub fn is_cgb_mode(&self) -> bool {
        self.cgb.mode
    }

    /// Check if double-speed CPU mode is active.
    pub fn is_double_speed(&self) -> bool {
        self.cgb.double_speed
    }

    /// Toggle double-speed mode (called by STOP opcode when KEY1 bit 0 is set).
    pub fn toggle_double_speed(&mut self) {
        self.cgb.toggle_double_speed();
    }

    /// Read a byte directly from a specific VRAM bank (PPU bank-independent access).
    pub(crate) fn read_vram_bank(&self, bank: usize, addr: u16) -> u8 {
        if (0x8000..0xA000).contains(&addr) {
            self.vram[bank & 1][(addr - 0x8000) as usize]
        } else {
            0xFF
        }
    }

    /// Read two bytes from the BG colour palette RAM (lo, hi) for palette + colour index.
    #[inline]
    pub(crate) fn read_bg_palette(&self, palette: usize, color: usize) -> (u8, u8) {
        self.cgb.read_bg_palette(palette, color)
    }

    /// Read two bytes from the OBJ colour palette RAM (lo, hi) for palette + colour index.
    #[inline]
    pub(crate) fn read_obj_palette(&self, palette: usize, color: usize) -> (u8, u8) {
        self.cgb.read_obj_palette(palette, color)
    }

    /// Perform one H-blank HDMA step: transfer 16 bytes from source to VRAM.
    pub fn tick_hdma_hblank(&mut self) {
        if !self.cgb.hdma_active || !self.cgb.hdma_hblank {
            return;
        }
        for i in 0..16u16 {
            let byte = self.read(self.cgb.hdma_source + i);
            let dest_vram = (self.cgb.hdma_dest & 0x1FFF) + i;
            self.vram[self.cgb.vram_bank][dest_vram as usize] = byte;
        }
        self.cgb.hdma_source += 16;
        self.cgb.hdma_dest += 16;
        if self.cgb.hdma_len == 0 {
            self.cgb.hdma_active = false;
            self.io[0x55] = 0xFF;
        } else {
            self.cgb.hdma_len -= 1;
            self.io[0x55] = self.cgb.hdma_len;
        }
    }

    // ── Camera accessors (delegates to PocketCamera cartridge) ──────────────

    pub fn set_camera_image(&mut self, data: &[u8]) {
        if let Some(cam) = self.cartridge.as_camera_mut() {
            cam.set_image(data);
        }
    }

    pub fn is_camera_image_ready(&self) -> bool {
        self.cartridge
            .as_camera()
            .map(|c| c.is_image_ready())
            .unwrap_or(false)
    }

    pub fn is_camera_capture_dirty(&self) -> bool {
        self.cartridge
            .as_camera()
            .map(|c| c.is_capture_dirty())
            .unwrap_or(false)
    }

    pub fn clear_camera_capture_dirty(&mut self) {
        if let Some(cam) = self.cartridge.as_camera_mut() {
            cam.clear_capture_dirty();
        }
    }

    pub fn camera_capture_sram(&self) -> &[u8] {
        static EMPTY: [u8; 0] = [];
        self.cartridge
            .as_camera()
            .map(|c| c.capture_sram())
            .unwrap_or(&EMPTY)
    }

    pub fn decode_camera_photo(&self, slot: u8) -> Vec<u8> {
        self.cartridge
            .as_camera()
            .map(|c| c.decode_photo(slot))
            .unwrap_or_default()
    }

    pub fn encode_camera_photo(&mut self, slot: u8, rgba: &[u8]) -> bool {
        self.cartridge
            .as_camera_mut()
            .map(|c| c.encode_photo(slot, rgba))
            .unwrap_or(false)
    }

    pub fn clear_camera_photo_slot(&mut self, slot: u8) {
        if let Some(cam) = self.cartridge.as_camera_mut() {
            cam.clear_photo_slot(slot);
        }
    }

    pub fn camera_contrast(&self) -> i32 {
        self.cartridge
            .as_camera()
            .map(|c| c.contrast())
            .unwrap_or(-1)
    }

    #[cfg_attr(not(any(feature = "ios", feature = "wasm")), allow(dead_code))]
    pub fn camera_photo_count(&self) -> u8 {
        self.cartridge
            .as_camera()
            .map(|c| c.photo_count())
            .unwrap_or(0)
    }

    // ── MBC7 accelerometer accessor ──────────────────────────────────────────

    /// Feed accelerometer data to an MBC7 cartridge (Kirby's Tilt 'n' Tumble).
    /// No-op for all other cartridge types.
    pub fn set_accelerometer(&mut self, x: i32, y: i32) {
        if let Some(m) = self.cartridge.as_mbc7_mut() {
            m.set_accelerometer(x, y);
        }
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

    /// Helper: create a minimal ROM (0x8000 bytes) with given cart type and RAM size byte.
    fn make_rom(cart_type: u8, ram_size_byte: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = cart_type;
        rom[0x0149] = ram_size_byte;
        rom
    }

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
        assert_eq!(mem.read(0xFEA0), 0xFF);
        assert_eq!(mem.read(0xFEFF), 0xFF);
    }

    #[test]
    fn test_rom_bank_switching() {
        let mut mem = Memory::new();

        let mut rom = vec![0u8; 0x8000];
        rom[0x0000] = 0x11;
        rom[0x4000] = 0x22;

        mem.load_rom(&rom, false).unwrap();

        assert_eq!(mem.read(0x0000), 0x11);
        assert_eq!(mem.read(0x4000), 0x22);
    }

    #[test]
    fn test_rom_bank_select() {
        let mut mem = Memory::new();

        let mut rom = vec![0u8; 0x10000];
        rom[0x0147] = 0x01; // MBC1
        rom[0x4000] = 0x11; // Bank 1
        rom[0x8000] = 0x22; // Bank 2
        rom[0xC000] = 0x33; // Bank 3

        mem.load_rom(&rom, false).unwrap();

        mem.write(0x2000, 0x02);
        assert_eq!(mem.read(0x4000), 0x22);

        mem.write(0x2000, 0x03);
        assert_eq!(mem.read(0x4000), 0x33);

        mem.write(0x2000, 0x00); // bank 0 → maps to bank 1
        assert_eq!(mem.read(0x4000), 0x11);
    }

    #[test]
    fn test_external_ram_enable() {
        let mut mem = Memory::new();
        // Use MBC1 + RAM cartridge so RAM enable works
        mem.load_rom(&make_rom(0x03, 0x02), false).unwrap(); // MBC1+RAM+BATTERY, 8KB

        // RAM disabled by default
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
        mem.write(0xFF04, 0x42);
        assert_eq!(mem.read(0xFF04), 0x00);
    }

    #[test]
    fn test_ly_readonly() {
        let mut mem = Memory::new();
        mem.write_io_direct(0x44, 0x50);
        mem.write(0xFF44, 0x99);
        assert_eq!(mem.read(0xFF44), 0x50);
    }

    #[test]
    fn test_dma_transfer() {
        let mut mem = Memory::new();
        for i in 0..0xA0 {
            mem.write(0xC000 + i as u16, i as u8);
        }
        mem.write(0xFF46, 0xC0);
        for i in 0..0xA0 {
            assert_eq!(mem.read(0xFE00 + i as u16), i as u8);
        }
    }

    #[test]
    fn test_load_rom_too_small() {
        let mut mem = Memory::new();
        let small_rom = vec![0u8; 0x100];
        assert!(mem.load_rom(&small_rom, false).is_err());
    }

    #[test]
    fn test_cartridge_ram_persistence() {
        let mut mem = Memory::new();
        // Use MBC1 + RAM so enable/disable works
        mem.load_rom(&make_rom(0x03, 0x02), false).unwrap();

        mem.write(0x0000, 0x0A); // Enable RAM
        mem.write(0xA000, 0x42);
        mem.write(0xA001, 0x43);

        let ram = mem.get_cartridge_ram().to_vec();
        assert_eq!(ram[0], 0x42);
        assert_eq!(ram[1], 0x43);

        let mut mem2 = Memory::new();
        mem2.load_rom(&make_rom(0x03, 0x02), false).unwrap();
        mem2.load_cartridge_ram(&ram);
        mem2.write(0x0000, 0x0A);
        assert_eq!(mem2.read(0xA000), 0x42);
        assert_eq!(mem2.read(0xA001), 0x43);
    }

    #[test]
    fn test_cgb_load_rom_sets_mode() {
        let mut mem = Memory::new();
        let rom = vec![0u8; 0x8000];
        mem.load_rom(&rom, true).unwrap();
        assert!(mem.is_cgb_mode());

        mem.load_rom(&rom, false).unwrap();
        assert!(!mem.is_cgb_mode());
    }

    #[test]
    fn test_cgb_vram_bank_switching() {
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], true).unwrap(); // CGB mode

        mem.write(0x8000, 0xAA);
        assert_eq!(mem.read(0x8000), 0xAA);

        // Switch to VRAM bank 1
        mem.write(0xFF4F, 0x01);
        assert_eq!(mem.read(0xFF4F), 0xFF); // bit 0 set, other bits = 1

        mem.write(0x8000, 0xBB);
        assert_eq!(mem.read(0x8000), 0xBB);

        // Switch back to bank 0
        mem.write(0xFF4F, 0x00);
        assert_eq!(mem.read(0x8000), 0xAA);

        assert_eq!(mem.read_vram_bank(0, 0x8000), 0xAA);
        assert_eq!(mem.read_vram_bank(1, 0x8000), 0xBB);
    }

    #[test]
    fn test_cgb_wram_bank_switching() {
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], true).unwrap(); // CGB mode

        mem.write(0xC100, 0x11);
        mem.write(0xD000, 0x22); // default switchable bank = 1

        // Switch to bank 3
        mem.write(0xFF70, 0x03);
        assert_eq!(mem.read(0xFF70), 0x03 | 0xF8);

        mem.write(0xD000, 0x33);
        assert_eq!(mem.read(0xD000), 0x33);

        // Back to bank 1
        mem.write(0xFF70, 0x01);
        assert_eq!(mem.read(0xD000), 0x22);

        assert_eq!(mem.read(0xC100), 0x11);
    }

    #[test]
    fn test_cgb_bg_palette_write_read() {
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], true).unwrap(); // CGB mode

        mem.write(0xFF68, 0x00);
        mem.write(0xFF69, 0xFF);
        mem.write(0xFF68, 0x01);
        mem.write(0xFF69, 0x7F);

        let (lo, hi) = mem.read_bg_palette(0, 0);
        assert_eq!(lo, 0xFF, "palette lo byte");
        assert_eq!(hi, 0x7F, "palette hi byte");
    }

    #[test]
    fn test_cgb_obj_palette_auto_increment() {
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], true).unwrap(); // CGB mode

        mem.write(0xFF6A, 0x80); // OCPS auto-increment at address 0

        let bytes = [0x00u8, 0x00, 0xFF, 0x7F, 0x1F, 0x00, 0xFF, 0x00];
        for b in bytes {
            mem.write(0xFF6B, b);
        }

        let ocps = mem.read(0xFF6A);
        assert_eq!(ocps & 0x3F, 8, "OCPS address after 8 auto-increments");

        let (lo, hi) = mem.read_obj_palette(0, 1);
        assert_eq!(lo, 0xFF);
        assert_eq!(hi, 0x7F);
    }

    #[test]
    fn test_cgb_key1_arm_and_toggle() {
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], true).unwrap(); // CGB mode

        assert!(!mem.is_double_speed());
        let key1 = mem.read(0xFF4D);
        assert_eq!(key1 & 0x01, 0, "speed_armed initially clear");
        assert_eq!(key1 & 0x80, 0, "double_speed initially clear");

        mem.write(0xFF4D, 0x01);
        let key1 = mem.read(0xFF4D);
        assert_eq!(key1 & 0x01, 1, "speed_armed set");

        mem.toggle_double_speed();
        assert!(mem.is_double_speed());

        let key1 = mem.read(0xFF4D);
        assert_eq!(key1 & 0x80, 0x80, "bit 7 reflects double_speed");
        assert_eq!(key1 & 0x01, 0, "speed_armed cleared after toggle");

        mem.write(0xFF4D, 0x01);
        mem.toggle_double_speed();
        assert!(!mem.is_double_speed());
    }

    #[test]
    fn test_dmg_ignores_cgb_registers() {
        // In DMG mode, GBC-only registers should return 0xFF on read
        // and silently discard writes.
        let mut mem = Memory::new();
        mem.load_rom(&vec![0u8; 0x8000], false).unwrap(); // DMG mode

        // VBK write should be ignored — VRAM stays on bank 0
        mem.write(0x8000, 0xAA);
        mem.write(0xFF4F, 0x01); // attempt VBK switch
        mem.write(0x8000, 0xBB); // should write to bank 0 (still)
        assert_eq!(mem.read(0x8000), 0xBB);
        assert_eq!(mem.read_vram_bank(0, 0x8000), 0xBB);
        assert_eq!(mem.read_vram_bank(1, 0x8000), 0x00); // bank 1 untouched

        // GBC registers return 0xFF in DMG mode
        assert_eq!(mem.read(0xFF4D), 0xFF); // KEY1
        assert_eq!(mem.read(0xFF4F), 0xFF); // VBK
        assert_eq!(mem.read(0xFF68), 0xFF); // BCPS
        assert_eq!(mem.read(0xFF70), 0xFF); // SVBK
    }
}
