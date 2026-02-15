//! Pixel Processing Unit (PPU) emulation.
//!
//! Renders the 160x144 display by cycling through four modes per scanline:
//! OAM scan, pixel drawing, H-blank, and V-blank. Supports background tiles,
//! window overlay, and up to 10 sprites per scanline with priority sorting.
//!
//! Rendering is split by hardware mode:
//! - [`dmg`]: original Game Boy grayscale scanline rendering
//! - [`gbc`]: Game Boy Color colour palette + VRAM banking rendering

mod dmg;
mod gbc;

use std::fmt;

use crate::interrupts::{Interrupt, InterruptController};
use crate::memory::Memory;
use crate::memory::io;

/// Debug state for PPU inspection.
#[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: ppu_* accessors
pub struct PpuDebugState {
    pub mode: u8,
    pub mode_name: &'static str,
    pub line: u8,
    pub cycles: u32,
    pub window_line_counter: u8,
}

impl fmt::Display for PpuDebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mode={}({}) line={} cycles={} win_line={}",
            self.mode, self.mode_name, self.line, self.cycles, self.window_line_counter
        )
    }
}

impl PpuMode {
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: PpuDebugState
    fn name(self) -> &'static str {
        match self {
            PpuMode::HBlank => "HBLANK",
            PpuMode::VBlank => "VBLANK",
            PpuMode::OamScan => "OAM",
            PpuMode::Drawing => "DRAW",
        }
    }
}

pub(super) const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;
const VBLANK_LINES: usize = 10;
const TOTAL_LINES: usize = SCREEN_HEIGHT + VBLANK_LINES;

/// PPU operating modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PpuMode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

// Mode durations in cycles
const OAM_SCAN_CYCLES: u32 = 80;
const DRAWING_CYCLES: u32 = 172; // Variable, but we use fixed for simplicity
const HBLANK_CYCLES: u32 = 204;
const SCANLINE_CYCLES: u32 = 456;

pub struct Ppu {
    /// RGBA frame buffer — 160×144×4 bytes written directly by render functions.
    pub(super) buffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4]>,
    /// Per-pixel BG info for the current scanline — used for sprite priority.
    /// Bit 0 = pixel is BG colour 0 (transparent for sprites).
    /// Bit 1 = tile has GBC force-priority flag set.
    pub(super) scanline_bg_info: [u8; SCREEN_WIDTH],
    mode: PpuMode,
    pub(super) cycles: u32,
    pub(super) line: u8,
    pub(super) window_line_counter: u8,
    pub(crate) frame_ready: bool,
    /// GBC colour mode — set once at load_rom time, never changes mid-session.
    pub(super) cgb_mode: bool,
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            buffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT * 4]),
            scanline_bg_info: [0; SCREEN_WIDTH],
            mode: PpuMode::OamScan,
            cycles: 0,
            line: 0,
            window_line_counter: 0,
            frame_ready: false,
            cgb_mode: false,
        }
    }

    /// Reset PPU to power-on state for the given mode.
    /// Called by GameBoyCore::load_rom() on every ROM load.
    pub fn reset(&mut self, cgb_mode: bool) {
        *self = Self::new();
        self.cgb_mode = cgb_mode;
    }

    pub fn tick(&mut self, cycles: u32, memory: &mut Memory, interrupts: &InterruptController) {
        let lcdc = memory.read_io_direct(io::LCDC);

        // LCD disabled - keep the last frame visible (don't clear buffer)
        if lcdc & 0x80 == 0 {
            self.mode = PpuMode::HBlank;
            self.cycles = 0;
            self.line = 0;
            memory.write_io_direct(io::LY, 0);
            return;
        }

        self.cycles += cycles;

        match self.mode {
            PpuMode::OamScan => {
                if self.cycles >= OAM_SCAN_CYCLES {
                    self.cycles -= OAM_SCAN_CYCLES;
                    self.mode = PpuMode::Drawing;
                }
            }
            PpuMode::Drawing => {
                if self.cycles >= DRAWING_CYCLES {
                    self.cycles -= DRAWING_CYCLES;
                    self.mode = PpuMode::HBlank;

                    self.render_scanline(memory);

                    let stat = memory.read_io_direct(io::STAT);
                    if stat & 0x08 != 0 {
                        interrupts.request(Interrupt::LcdStat, memory);
                    }
                }
            }
            PpuMode::HBlank => {
                if self.cycles >= HBLANK_CYCLES {
                    self.cycles -= HBLANK_CYCLES;
                    self.line += 1;
                    memory.write_io_direct(io::LY, self.line);

                    self.check_lyc_coincidence(memory, interrupts);

                    if self.line >= SCREEN_HEIGHT as u8 {
                        self.mode = PpuMode::VBlank;
                        self.window_line_counter = 0;
                        self.frame_ready = true;
                        interrupts.request(Interrupt::VBlank, memory);

                        let stat = memory.read_io_direct(io::STAT);
                        if stat & 0x10 != 0 {
                            interrupts.request(Interrupt::LcdStat, memory);
                        }
                    } else {
                        self.mode = PpuMode::OamScan;

                        let stat = memory.read_io_direct(io::STAT);
                        if stat & 0x20 != 0 {
                            interrupts.request(Interrupt::LcdStat, memory);
                        }
                    }
                }
            }
            PpuMode::VBlank => {
                if self.cycles >= SCANLINE_CYCLES {
                    self.cycles -= SCANLINE_CYCLES;
                    self.line += 1;

                    if self.line >= TOTAL_LINES as u8 {
                        self.line = 0;
                        self.mode = PpuMode::OamScan;

                        let stat = memory.read_io_direct(io::STAT);
                        if stat & 0x20 != 0 {
                            interrupts.request(Interrupt::LcdStat, memory);
                        }
                    }

                    memory.write_io_direct(io::LY, self.line);
                    self.check_lyc_coincidence(memory, interrupts);
                }
            }
        }

        // Update STAT register mode bits
        let mut stat = memory.read_io_direct(io::STAT);
        stat = (stat & 0xFC) | self.mode as u8;
        memory.write_io_direct(io::STAT, stat);
    }

    fn check_lyc_coincidence(&self, memory: &mut Memory, interrupts: &InterruptController) {
        let lyc = memory.read_io_direct(io::LYC);
        let mut stat = memory.read_io_direct(io::STAT);

        if self.line == lyc {
            stat |= 0x04;
            if stat & 0x40 != 0 {
                interrupts.request(Interrupt::LcdStat, memory);
            }
        } else {
            stat &= !0x04;
        }

        memory.write_io_direct(io::STAT, stat);
    }

    fn render_scanline(&mut self, memory: &Memory) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let line = self.line as usize;

        if line >= SCREEN_HEIGHT {
            return;
        }

        // Default: every pixel treated as BG colour 0 (transparent for sprites)
        self.scanline_bg_info.fill(0x01);

        // Background
        if lcdc & 0x01 != 0 {
            if self.cgb_mode {
                self.render_background_gbc(memory, line);
            } else {
                self.render_background_dmg(memory, line);
            }
        } else {
            // Background disabled — fill scanline with white
            let start = line * SCREEN_WIDTH * 4;
            for px in 0..SCREEN_WIDTH {
                self.buffer[start + px * 4..start + px * 4 + 4]
                    .copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
            }
        }

        // Window
        if lcdc & 0x20 != 0 {
            if self.cgb_mode {
                self.render_window_gbc(memory, line);
            } else {
                self.render_window_dmg(memory, line);
            }
        }

        // Sprites
        if lcdc & 0x02 != 0 {
            if self.cgb_mode {
                self.render_sprites_gbc(memory, line);
            } else {
                self.render_sprites_dmg(memory, line);
            }
        }
    }

    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: step_single
    pub fn frame_ready(&mut self) -> bool {
        let r = self.frame_ready;
        self.frame_ready = false;
        r
    }

    pub fn get_buffer(&self) -> &[u8] {
        &*self.buffer
    }

    /// Get current PPU state for debugging.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: ppu_* accessors
    pub fn get_debug_state(&self) -> PpuDebugState {
        PpuDebugState {
            mode: self.mode as u8,
            mode_name: self.mode.name(),
            line: self.line,
            cycles: self.cycles,
            window_line_counter: self.window_line_counter,
        }
    }

    /// Count non-zero bytes in the buffer (useful for debug/test assertions).
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))] // wasm: log_frame_debug
    pub fn count_non_zero_pixels(&self) -> usize {
        self.buffer.iter().filter(|&&p| p != 0).count()
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_initial_state() {
        let ppu = Ppu::new();
        assert_eq!(ppu.mode, PpuMode::OamScan);
        assert_eq!(ppu.line, 0);
        assert_eq!(ppu.cycles, 0);
    }

    #[test]
    fn test_buffer_size() {
        let ppu = Ppu::new();
        assert_eq!(ppu.get_buffer().len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    }
}
