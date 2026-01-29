//! Pixel Processing Unit (PPU) emulation.
//!
//! Renders the 160x144 display by cycling through four modes per scanline:
//! OAM scan, pixel drawing, H-blank, and V-blank. Supports background tiles,
//! window overlay, and up to 10 sprites per scanline with priority sorting.

use std::fmt;

use crate::interrupts::{Interrupt, InterruptController};
use crate::memory::io;
use crate::memory::Memory;

/// Debug state for PPU inspection.
#[allow(dead_code)]
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
    #[allow(dead_code)]
    fn name(self) -> &'static str {
        match self {
            PpuMode::HBlank => "HBLANK",
            PpuMode::VBlank => "VBLANK",
            PpuMode::OamScan => "OAM",
            PpuMode::Drawing => "DRAW",
        }
    }
}

const SCREEN_WIDTH: usize = 160;
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
    buffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT]>,
    mode: PpuMode,
    cycles: u32,
    line: u8,
    window_line_counter: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            buffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT]),
            mode: PpuMode::OamScan,
            cycles: 0,
            line: 0,
            window_line_counter: 0,
        }
    }

    pub fn tick(&mut self, cycles: u32, memory: &mut Memory, interrupts: &InterruptController) {
        let lcdc = memory.read_io_direct(io::LCDC);

        // LCD disabled - keep the last frame visible (don't clear buffer)
        if lcdc & 0x80 == 0 {
            self.mode = PpuMode::HBlank;
            self.cycles = 0;
            self.line = 0;
            memory.write_io_direct(io::LY, 0); // LY = 0
            // Note: We don't clear the buffer here, so last frame stays visible
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

                    // Render scanline
                    self.render_scanline(memory);

                    // STAT interrupt for HBLANK
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

                    // Check LYC coincidence
                    self.check_lyc_coincidence(memory, interrupts);

                    if self.line >= SCREEN_HEIGHT as u8 {
                        self.mode = PpuMode::VBlank;
                        self.window_line_counter = 0;
                        interrupts.request(Interrupt::VBlank, memory);

                        // STAT interrupt for VBLANK
                        let stat = memory.read_io_direct(io::STAT);
                        if stat & 0x10 != 0 {
                            interrupts.request(Interrupt::LcdStat, memory);
                        }
                    } else {
                        self.mode = PpuMode::OamScan;

                        // STAT interrupt for OAM
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

                        // STAT interrupt for OAM
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
            stat |= 0x04; // Set coincidence flag
            if stat & 0x40 != 0 {
                interrupts.request(Interrupt::LcdStat, memory);
            }
        } else {
            stat &= !0x04; // Clear coincidence flag
        }

        memory.write_io_direct(io::STAT, stat);
    }

    fn render_scanline(&mut self, memory: &Memory) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let line = self.line as usize;

        if line >= SCREEN_HEIGHT {
            return;
        }

        // Background
        if lcdc & 0x01 != 0 {
            self.render_background(memory, line);
        } else {
            // Background disabled — clear scanline
            let start = line * SCREEN_WIDTH;
            self.buffer[start..start + SCREEN_WIDTH].fill(0);
        }

        // Window enabled
        if lcdc & 0x20 != 0 {
            self.render_window(memory, line);
        }

        // Sprites enabled
        if lcdc & 0x02 != 0 {
            self.render_sprites(memory, line);
        }
    }

    fn render_background(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let scy = memory.read_io_direct(io::SCY) as usize;
        let scx = memory.read_io_direct(io::SCX) as usize;
        let bgp = memory.read_io_direct(io::BGP);

        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x08 != 0 { 0x9C00 } else { 0x9800 };
        let signed_addressing = lcdc & 0x10 == 0;

        let y = (line + scy) & 0xFF;
        let tile_row = y / 8;
        let pixel_row = y % 8;
        let pixel_row_offset = pixel_row as u16 * 2;

        let buf = &mut self.buffer[line * SCREEN_WIDTH..][..SCREEN_WIDTH];

        for screen_x in 0..SCREEN_WIDTH {
            let x = (screen_x + scx) & 0xFF;
            let tile_col = x >> 3;
            let pixel_col = 7 - (x & 7);

            let tile_idx = memory.read(tile_map_base + (tile_row * 32 + tile_col) as u16);

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);

            let color_idx = ((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1);
            buf[screen_x] = (bgp >> (color_idx * 2)) & 0x03;
        }
    }

    fn render_window(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let wy = memory.read_io_direct(io::WY) as usize;
        let wx = memory.read_io_direct(io::WX) as i16 - 7;
        let bgp = memory.read_io_direct(io::BGP);

        if line < wy || wx >= SCREEN_WIDTH as i16 {
            return;
        }

        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x40 != 0 { 0x9C00 } else { 0x9800 };
        let signed_addressing = lcdc & 0x10 == 0;

        let window_y = self.window_line_counter as usize;
        let tile_row = window_y / 8;
        let pixel_row = window_y % 8;
        let pixel_row_offset = pixel_row as u16 * 2;

        let start_x = wx.max(0) as usize;
        let buf = &mut self.buffer[line * SCREEN_WIDTH..][..SCREEN_WIDTH];

        for screen_x in start_x..SCREEN_WIDTH {
            let window_x = (screen_x as i16 - wx) as usize;
            let tile_col = window_x >> 3;
            let pixel_col = 7 - (window_x & 7);

            let tile_idx = memory.read(tile_map_base + (tile_row * 32 + tile_col) as u16);

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);

            let color_idx = ((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1);
            buf[screen_x] = (bgp >> (color_idx * 2)) & 0x03;
        }

        self.window_line_counter += 1;
    }

    fn render_sprites(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let sprite_height: i16 = if lcdc & 0x04 != 0 { 16 } else { 8 };
        let oam = memory.get_oam();

        // Cache palette registers outside the sprite loop
        let obp0 = memory.read_io_direct(io::OBP0);
        let obp1 = memory.read_io_direct(io::OBP1);

        // Collect sprites on this line (max 10, stack-allocated)
        let mut sprites: [(u8, i16, u8, u8); 10] = [(0, 0, 0, 0); 10];
        let mut sprite_count: usize = 0;

        for i in 0..40 {
            let offset = i * 4;
            let oam_y = oam[offset] as i16;
            let screen_y = oam_y - 16;
            let x = oam[offset + 1];
            let tile = oam[offset + 2];
            let flags = oam[offset + 3];

            if (line as i16) >= screen_y && (line as i16) < screen_y + sprite_height {
                sprites[sprite_count] = (x, screen_y, tile, flags);
                sprite_count += 1;
                if sprite_count >= 10 {
                    break;
                }
            }
        }

        // Sort by X coordinate (lower X = higher priority)
        sprites[..sprite_count].sort_by(|a, b| a.0.cmp(&b.0));

        // Render sprites (reverse order so higher priority overwrites)
        for &(x, screen_y, mut tile, flags) in sprites[..sprite_count].iter().rev() {
            let palette = if flags & 0x10 != 0 { obp1 } else { obp0 };

            let flip_x = flags & 0x20 != 0;
            let flip_y = flags & 0x40 != 0;
            let bg_priority = flags & 0x80 != 0;

            let mut sprite_row = (line as i16) - screen_y;
            if flip_y {
                sprite_row = sprite_height - 1 - sprite_row;
            }

            // For 8x16 sprites, adjust tile index
            if sprite_height == 16 {
                tile &= 0xFE;
                if sprite_row >= 8 {
                    tile += 1;
                    sprite_row -= 8;
                }
            }

            let tile_addr = 0x8000 + tile as u16 * 16 + sprite_row as u16 * 2;
            let low = memory.read(tile_addr);
            let high = memory.read(tile_addr + 1);

            for pixel in 0..8i16 {
                let screen_x = x as i16 - 8 + pixel;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 {
                    continue;
                }

                let bit = if flip_x { pixel } else { 7 - pixel };
                let color_idx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);

                // Color 0 is transparent for sprites
                if color_idx == 0 {
                    continue;
                }

                let buffer_idx = line * SCREEN_WIDTH + screen_x as usize;

                // BG priority: sprite only shows over BG color 0
                if bg_priority && self.buffer[buffer_idx] != 0 {
                    continue;
                }

                self.buffer[buffer_idx] = (palette >> (color_idx * 2)) & 0x03;
            }
        }
    }

    pub fn get_buffer(&self) -> &[u8] {
        &*self.buffer
    }

    /// Get current PPU state for debugging.
    #[allow(dead_code)]
    pub fn get_debug_state(&self) -> PpuDebugState {
        PpuDebugState {
            mode: self.mode as u8,
            mode_name: self.mode.name(),
            line: self.line,
            cycles: self.cycles,
            window_line_counter: self.window_line_counter,
        }
    }

    /// Count non-zero pixels in the buffer.
    #[allow(dead_code)]
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
        assert_eq!(ppu.get_buffer().len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    }
}
