//! DMG (original Game Boy) scanline rendering.
//!
//! All methods write RGBA directly to `self.buffer` and update
//! `self.scanline_bg_info` for downstream sprite priority checks.

use crate::memory::io;
use crate::memory::Memory;
use super::{Ppu, SCREEN_WIDTH};

impl Ppu {
    pub(super) fn render_background_dmg(&mut self, memory: &Memory, line: usize) {
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

        for screen_x in 0..SCREEN_WIDTH {
            let x = (screen_x + scx) & 0xFF;
            let tile_col = x >> 3;
            let tile_map_addr = tile_map_base + (tile_row * 32 + tile_col) as u16;
            let pixel_col = 7 - (x & 7);

            let tile_idx = memory.read(tile_map_addr);
            let pixel_row_offset = pixel_row as u16 * 2;
            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);
            let color_idx = ((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1);
            let shade = (bgp >> (color_idx * 2)) & 0x03;
            const GRAY: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
            let g = GRAY[shade as usize];
            let offset = (line * SCREEN_WIDTH + screen_x) * 4;
            self.buffer[offset..offset + 4].copy_from_slice(&[g, g, g, 255]);
            self.scanline_bg_info[screen_x] = (color_idx == 0) as u8;
        }
    }

    pub(super) fn render_window_dmg(&mut self, memory: &Memory, line: usize) {
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
        let start_x = wx.max(0) as usize;

        for screen_x in start_x..SCREEN_WIDTH {
            let window_x = (screen_x as i16 - wx) as usize;
            let tile_col = window_x >> 3;
            let tile_map_addr = tile_map_base + (tile_row * 32 + tile_col) as u16;
            let pixel_col = 7 - (window_x & 7);

            let tile_idx = memory.read(tile_map_addr);
            let pixel_row_offset = pixel_row as u16 * 2;
            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let low = memory.read(tile_data_addr);
            let high = memory.read(tile_data_addr + 1);
            let color_idx = ((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1);
            let shade = (bgp >> (color_idx * 2)) & 0x03;
            const GRAY: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
            let g = GRAY[shade as usize];
            let offset = (line * SCREEN_WIDTH + screen_x) * 4;
            self.buffer[offset..offset + 4].copy_from_slice(&[g, g, g, 255]);
            self.scanline_bg_info[screen_x] = (color_idx == 0) as u8;
        }

        self.window_line_counter += 1;
    }

    pub(super) fn render_sprites_dmg(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let sprite_height: i16 = if lcdc & 0x04 != 0 { 16 } else { 8 };
        let oam = memory.get_oam();
        let obp0 = memory.read_io_direct(io::OBP0);
        let obp1 = memory.read_io_direct(io::OBP1);

        let mut sprites: [(u8, i16, u8, u8); 10] = [(0, 0, 0, 0); 10];
        let mut sprite_count: usize = 0;

        for i in 0..40 {
            let o = i * 4;
            let screen_y = oam[o] as i16 - 16;
            if (line as i16) >= screen_y && (line as i16) < screen_y + sprite_height {
                sprites[sprite_count] = (oam[o + 1], screen_y, oam[o + 2], oam[o + 3]);
                sprite_count += 1;
                if sprite_count >= 10 {
                    break;
                }
            }
        }

        sprites[..sprite_count].sort_by_key(|s| s.0);

        const GRAY: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];

        for &(x, screen_y, mut tile, flags) in sprites[..sprite_count].iter().rev() {
            let flip_x = flags & 0x20 != 0;
            let flip_y = flags & 0x40 != 0;
            let bg_priority = flags & 0x80 != 0;

            let mut sprite_row = (line as i16) - screen_y;
            if flip_y {
                sprite_row = sprite_height - 1 - sprite_row;
            }

            if sprite_height == 16 {
                tile &= 0xFE;
                if sprite_row >= 8 {
                    tile += 1;
                    sprite_row -= 8;
                }
            }

            let tile_addr = 0x8000u16 + tile as u16 * 16 + sprite_row as u16 * 2;
            let low = memory.read_vram_bank(0, tile_addr);
            let high = memory.read_vram_bank(0, tile_addr + 1);

            for pixel in 0..8i16 {
                let screen_x = x as i16 - 8 + pixel;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 {
                    continue;
                }
                let sx = screen_x as usize;
                let bit = if flip_x { pixel as u8 } else { 7 - pixel as u8 };
                let color_idx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);

                if color_idx == 0 {
                    continue;
                }

                // bg_priority: sprite hides behind non-colour-0 BG pixels
                if bg_priority && (self.scanline_bg_info[sx] & 0x01 == 0) {
                    continue;
                }

                let palette = if flags & 0x10 != 0 { obp1 } else { obp0 };
                let shade = (palette >> (color_idx * 2)) & 0x03;
                let g = GRAY[shade as usize];
                let offset = (line * SCREEN_WIDTH + sx) * 4;
                self.buffer[offset..offset + 4].copy_from_slice(&[g, g, g, 255]);
            }
        }
    }
}
