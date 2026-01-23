use std::fmt;

use crate::interrupts::InterruptController;
use crate::memory::Memory;

/// Debug state for PPU inspection.
pub struct PpuDebugState {
    pub mode: u8,
    pub line: u8,
    pub cycles: u32,
    pub window_line_counter: u8,
}

impl fmt::Display for PpuDebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode_name = match self.mode {
            0 => "HBLANK",
            1 => "VBLANK",
            2 => "OAM",
            3 => "DRAW",
            _ => "???",
        };
        write!(
            f,
            "mode={}({}) line={} cycles={} win_line={}",
            self.mode, mode_name, self.line, self.cycles, self.window_line_counter
        )
    }
}

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;
const VBLANK_LINES: usize = 10;
const TOTAL_LINES: usize = SCREEN_HEIGHT + VBLANK_LINES;

// PPU modes
const MODE_HBLANK: u8 = 0;
const MODE_VBLANK: u8 = 1;
const MODE_OAM_SCAN: u8 = 2;
const MODE_DRAWING: u8 = 3;

// Mode durations in cycles
const OAM_SCAN_CYCLES: u32 = 80;
const DRAWING_CYCLES: u32 = 172; // Variable, but we use fixed for simplicity
const HBLANK_CYCLES: u32 = 204;
const SCANLINE_CYCLES: u32 = 456;

pub struct Ppu {
    buffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT]>,
    mode: u8,
    cycles: u32,
    line: u8,
    window_line_counter: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            buffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT]),
            mode: MODE_OAM_SCAN,
            cycles: 0,
            line: 0,
            window_line_counter: 0,
        }
    }

    pub fn tick(&mut self, cycles: u32, memory: &mut Memory, interrupts: &InterruptController) {
        let lcdc = memory.read_io_direct(0x40);

        // LCD disabled - keep the last frame visible (don't clear buffer)
        if lcdc & 0x80 == 0 {
            self.mode = MODE_HBLANK;
            self.cycles = 0;
            self.line = 0;
            memory.write_io_direct(0x44, 0); // LY = 0
            // Note: We don't clear the buffer here, so last frame stays visible
            return;
        }

        self.cycles += cycles;

        match self.mode {
            MODE_OAM_SCAN => {
                if self.cycles >= OAM_SCAN_CYCLES {
                    self.cycles -= OAM_SCAN_CYCLES;
                    self.mode = MODE_DRAWING;
                }
            }
            MODE_DRAWING => {
                if self.cycles >= DRAWING_CYCLES {
                    self.cycles -= DRAWING_CYCLES;
                    self.mode = MODE_HBLANK;

                    // Render scanline
                    self.render_scanline(memory);

                    // STAT interrupt for HBLANK
                    let stat = memory.read_io_direct(0x41);
                    if stat & 0x08 != 0 {
                        interrupts.request_lcd_stat(memory);
                    }
                }
            }
            MODE_HBLANK => {
                if self.cycles >= HBLANK_CYCLES {
                    self.cycles -= HBLANK_CYCLES;
                    self.line += 1;
                    memory.write_io_direct(0x44, self.line);

                    // Check LYC coincidence
                    self.check_lyc_coincidence(memory, interrupts);

                    if self.line >= SCREEN_HEIGHT as u8 {
                        self.mode = MODE_VBLANK;
                        self.window_line_counter = 0;
                        interrupts.request_vblank(memory);

                        // STAT interrupt for VBLANK
                        let stat = memory.read_io_direct(0x41);
                        if stat & 0x10 != 0 {
                            interrupts.request_lcd_stat(memory);
                        }
                    } else {
                        self.mode = MODE_OAM_SCAN;

                        // STAT interrupt for OAM
                        let stat = memory.read_io_direct(0x41);
                        if stat & 0x20 != 0 {
                            interrupts.request_lcd_stat(memory);
                        }
                    }
                }
            }
            MODE_VBLANK => {
                if self.cycles >= SCANLINE_CYCLES {
                    self.cycles -= SCANLINE_CYCLES;
                    self.line += 1;

                    if self.line >= TOTAL_LINES as u8 {
                        self.line = 0;
                        self.mode = MODE_OAM_SCAN;

                        // STAT interrupt for OAM
                        let stat = memory.read_io_direct(0x41);
                        if stat & 0x20 != 0 {
                            interrupts.request_lcd_stat(memory);
                        }
                    }

                    memory.write_io_direct(0x44, self.line);
                    self.check_lyc_coincidence(memory, interrupts);
                }
            }
            _ => {}
        }

        // Update STAT register mode bits
        let mut stat = memory.read_io_direct(0x41);
        stat = (stat & 0xFC) | self.mode;
        memory.write_io_direct(0x41, stat);
    }

    fn check_lyc_coincidence(&self, memory: &mut Memory, interrupts: &InterruptController) {
        let lyc = memory.read_io_direct(0x45);
        let mut stat = memory.read_io_direct(0x41);

        if self.line == lyc {
            stat |= 0x04; // Set coincidence flag
            if stat & 0x40 != 0 {
                interrupts.request_lcd_stat(memory);
            }
        } else {
            stat &= !0x04; // Clear coincidence flag
        }

        memory.write_io_direct(0x41, stat);
    }

    fn render_scanline(&mut self, memory: &Memory) {
        let lcdc = memory.read_io_direct(0x40);
        let line = self.line as usize;

        if line >= SCREEN_HEIGHT {
            return;
        }

        // Clear scanline
        let start = line * SCREEN_WIDTH;
        for pixel in &mut self.buffer[start..start + SCREEN_WIDTH] {
            *pixel = 0;
        }

        // Background enabled
        if lcdc & 0x01 != 0 {
            self.render_background(memory, line);
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
        let lcdc = memory.read_io_direct(0x40);
        let scy = memory.read_io_direct(0x42) as usize;
        let scx = memory.read_io_direct(0x43) as usize;
        let bgp = memory.read_io_direct(0x47);

        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x08 != 0 { 0x9C00 } else { 0x9800 };
        let signed_addressing = lcdc & 0x10 == 0;

        let y = (line + scy) & 0xFF;
        let tile_row = y / 8;
        let pixel_row = y % 8;

        for screen_x in 0..SCREEN_WIDTH {
            let x = (screen_x + scx) & 0xFF;
            let tile_col = x / 8;
            let pixel_col = 7 - (x % 8);

            let tile_map_addr = tile_map_base + (tile_row * 32 + tile_col) as u16;
            let tile_idx = memory.read(tile_map_addr);

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row as i16 * 2) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row as u16 * 2
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);

            let color_bit_low = (low >> pixel_col) & 1;
            let color_bit_high = (high >> pixel_col) & 1;
            let color_idx = (color_bit_high << 1) | color_bit_low;

            let color = (bgp >> (color_idx * 2)) & 0x03;
            self.buffer[line * SCREEN_WIDTH + screen_x] = color;
        }
    }

    fn render_window(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(0x40);
        let wy = memory.read_io_direct(0x4A) as usize;
        let wx = memory.read_io_direct(0x4B) as i16 - 7;
        let bgp = memory.read_io_direct(0x47);

        if line < wy || wx >= SCREEN_WIDTH as i16 {
            return;
        }

        let tile_data_base: u16 = if lcdc & 0x10 != 0 { 0x8000 } else { 0x8800 };
        let tile_map_base: u16 = if lcdc & 0x40 != 0 { 0x9C00 } else { 0x9800 };
        let signed_addressing = lcdc & 0x10 == 0;

        let window_y = self.window_line_counter as usize;
        let tile_row = window_y / 8;
        let pixel_row = window_y % 8;

        let start_x = wx.max(0) as usize;

        for screen_x in start_x..SCREEN_WIDTH {
            let window_x = (screen_x as i16 - wx) as usize;
            let tile_col = window_x / 8;
            let pixel_col = 7 - (window_x % 8);

            let tile_map_addr = tile_map_base + (tile_row * 32 + tile_col) as u16;
            let tile_idx = memory.read(tile_map_addr);

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row as i16 * 2) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row as u16 * 2
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);

            let color_bit_low = (low >> pixel_col) & 1;
            let color_bit_high = (high >> pixel_col) & 1;
            let color_idx = (color_bit_high << 1) | color_bit_low;

            let color = (bgp >> (color_idx * 2)) & 0x03;
            self.buffer[line * SCREEN_WIDTH + screen_x] = color;
        }

        self.window_line_counter += 1;
    }

    fn render_sprites(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(0x40);
        let sprite_height: i16 = if lcdc & 0x04 != 0 { 16 } else { 8 };
        let oam = memory.get_oam();

        // Collect sprites on this line (max 10)
        // Store (x, screen_y, tile, flags) where screen_y is already adjusted
        let mut sprites: Vec<(u8, i16, u8, u8)> = Vec::with_capacity(10);

        for i in 0..40 {
            let offset = i * 4;
            let oam_y = oam[offset] as i16;
            let screen_y = oam_y - 16; // Convert OAM Y to screen Y
            let x = oam[offset + 1];
            let tile = oam[offset + 2];
            let flags = oam[offset + 3];

            if (line as i16) >= screen_y && (line as i16) < screen_y + sprite_height {
                sprites.push((x, screen_y, tile, flags));
                if sprites.len() >= 10 {
                    break;
                }
            }
        }

        // Sort by X coordinate (lower X = higher priority)
        sprites.sort_by(|a, b| a.0.cmp(&b.0));

        // Render sprites (reverse order so higher priority overwrites)
        for (x, screen_y, mut tile, flags) in sprites.into_iter().rev() {
            let palette = if flags & 0x10 != 0 {
                memory.read_io_direct(0x49) // OBP1
            } else {
                memory.read_io_direct(0x48) // OBP0
            };

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

            for pixel in 0..8 {
                let screen_x = x as i16 - 8 + pixel;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 {
                    continue;
                }

                let bit = if flip_x { pixel } else { 7 - pixel };
                let color_bit_low = (low >> bit) & 1;
                let color_bit_high = (high >> bit) & 1;
                let color_idx = (color_bit_high << 1) | color_bit_low;

                // Color 0 is transparent for sprites
                if color_idx == 0 {
                    continue;
                }

                let buffer_idx = line * SCREEN_WIDTH + screen_x as usize;

                // BG priority: sprite only shows over BG color 0
                if bg_priority && self.buffer[buffer_idx] != 0 {
                    continue;
                }

                let color = (palette >> (color_idx * 2)) & 0x03;
                self.buffer[buffer_idx] = color;
            }
        }
    }

    pub fn get_buffer(&self) -> &[u8] {
        &*self.buffer
    }

    /// Get current PPU state for debugging.
    pub fn get_debug_state(&self) -> PpuDebugState {
        PpuDebugState {
            mode: self.mode,
            line: self.line,
            cycles: self.cycles,
            window_line_counter: self.window_line_counter,
        }
    }

    /// Count non-zero pixels in the buffer.
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
        assert_eq!(ppu.mode, MODE_OAM_SCAN);
        assert_eq!(ppu.line, 0);
        assert_eq!(ppu.cycles, 0);
    }

    #[test]
    fn test_buffer_size() {
        let ppu = Ppu::new();
        assert_eq!(ppu.get_buffer().len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    }
}
