//! GBC (Game Boy Color) scanline rendering.
//!
//! Reads tile attributes from VRAM bank 1, decodes RGB555 palette entries,
//! and enforces GBC sprite priority rules (force-priority, OAM bg-priority, LCDC master).

use super::{Ppu, SCREEN_WIDTH};
use crate::memory::Memory;
use crate::memory::io;

impl Ppu {
    /// Convert a 15-bit RGB555 little-endian pair to RGBA.
    #[inline]
    pub(super) fn rgb555_to_rgba(lo: u8, hi: u8) -> [u8; 4] {
        let r5 = lo & 0x1F;
        let g5 = ((lo >> 5) | (hi << 3)) & 0x1F;
        let b5 = (hi >> 2) & 0x1F;
        [r5 << 3 | r5 >> 2, g5 << 3 | g5 >> 2, b5 << 3 | b5 >> 2, 255]
    }

    pub(super) fn render_background_gbc(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        let scy = memory.read_io_direct(io::SCY) as usize;
        let scx = memory.read_io_direct(io::SCX) as usize;

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

            // Bank 0 = tile index, bank 1 = tile attributes
            let tile_idx = memory.read_vram_bank(0, tile_map_addr);
            let attr = memory.read_vram_bank(1, tile_map_addr);
            let palette = (attr & 0x07) as usize;
            let tile_bank = ((attr >> 3) & 1) as usize;
            let x_flip = attr & 0x20 != 0;
            let y_flip = attr & 0x40 != 0;
            let force_priority = attr & 0x80 != 0;

            let actual_row = if y_flip { 7 - pixel_row } else { pixel_row };
            let pixel_row_offset = actual_row as u16 * 2;

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let low = memory.read_vram_bank(tile_bank, tile_data_addr);
            let high = memory.read_vram_bank(tile_bank, tile_data_addr + 1);

            let pixel_col = if x_flip { x & 7 } else { 7 - (x & 7) };
            let color_idx = (((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1)) as usize;

            let (lo, hi) = memory.read_bg_palette(palette, color_idx);
            let rgba = Self::rgb555_to_rgba(lo, hi);
            let offset = (line * SCREEN_WIDTH + screen_x) * 4;
            self.buffer[offset..offset + 4].copy_from_slice(&rgba);
            self.scanline_bg_info[screen_x] =
                (color_idx == 0) as u8 | ((force_priority as u8) << 1);
        }
    }

    pub(super) fn render_window_gbc(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        if lcdc & 0x20 == 0 {
            return;
        }

        let wy = memory.read_io_direct(io::WY) as usize;
        let wx = memory.read_io_direct(io::WX) as i16 - 7;

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
            let window_x = screen_x - start_x;
            let tile_col = window_x >> 3;
            let tile_map_addr = tile_map_base + (tile_row * 32 + tile_col) as u16;

            let tile_idx = memory.read_vram_bank(0, tile_map_addr);
            let attr = memory.read_vram_bank(1, tile_map_addr);
            let palette = (attr & 0x07) as usize;
            let tile_bank = ((attr >> 3) & 1) as usize;
            let x_flip = attr & 0x20 != 0;
            let y_flip = attr & 0x40 != 0;
            let force_priority = attr & 0x80 != 0;

            let actual_row = if y_flip { 7 - pixel_row } else { pixel_row };
            let pixel_row_offset = actual_row as u16 * 2;

            let tile_data_addr = if signed_addressing {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + 0x800 + signed_idx * 16 + pixel_row_offset as i16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16 + pixel_row_offset
            };

            let pixel_col = if x_flip {
                window_x & 7
            } else {
                7 - (window_x & 7)
            };
            let low = memory.read_vram_bank(tile_bank, tile_data_addr);
            let high = memory.read_vram_bank(tile_bank, tile_data_addr + 1);
            let color_idx = (((high >> pixel_col) & 1) << 1 | ((low >> pixel_col) & 1)) as usize;

            let (lo, hi) = memory.read_bg_palette(palette, color_idx);
            let rgba = Self::rgb555_to_rgba(lo, hi);
            let offset = (line * SCREEN_WIDTH + screen_x) * 4;
            self.buffer[offset..offset + 4].copy_from_slice(&rgba);
            self.scanline_bg_info[screen_x] =
                (color_idx == 0) as u8 | ((force_priority as u8) << 1);
        }

        self.window_line_counter += 1;
    }

    pub(super) fn render_sprites_gbc(&mut self, memory: &Memory, line: usize) {
        let lcdc = memory.read_io_direct(io::LCDC);
        if lcdc & 0x02 == 0 {
            return;
        }

        let sprite_height: i16 = if lcdc & 0x04 != 0 { 16 } else { 8 };
        let oam = memory.get_oam();

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

        for &(x, screen_y, mut tile, flags) in sprites[..sprite_count].iter().rev() {
            let flip_x = flags & 0x20 != 0;
            let flip_y = flags & 0x40 != 0;
            let bg_priority = flags & 0x80 != 0;
            let tile_bank = ((flags >> 3) & 1) as usize;
            let cgb_palette = (flags & 0x07) as usize;

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
            let low = memory.read_vram_bank(tile_bank, tile_addr);
            let high = memory.read_vram_bank(tile_bank, tile_addr + 1);

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

                let bg_info = self.scanline_bg_info[sx];
                let show_sprite = if lcdc & 0x01 == 0 {
                    // LCDC master BG disable: sprites always win
                    true
                } else if bg_info & 0x02 != 0 {
                    // BG tile has force-priority: sprite only over colour-0 BG pixels
                    bg_info & 0x01 != 0
                } else if bg_priority {
                    // OAM bg-priority: sprite only over colour-0 BG pixels
                    bg_info & 0x01 != 0
                } else {
                    true
                };

                if !show_sprite {
                    continue;
                }

                let (lo, hi) = memory.read_obj_palette(cgb_palette, color_idx as usize);
                let offset = (line * SCREEN_WIDTH + sx) * 4;
                self.buffer[offset..offset + 4].copy_from_slice(&Self::rgb555_to_rgba(lo, hi));
            }
        }
    }
}
