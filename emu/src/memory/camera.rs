//! Game Boy Camera (Pocket Camera) sensor state and photo operations.
//!
//! ## SRAM layout (128KB = 16 banks × 8KB)
//!
//! | Offset        | Content                                      |
//! |---------------|----------------------------------------------|
//! | 0x00000-0x00FF| Camera sensor buffer / metadata               |
//! | 0x00100-0x00EFF| Active capture (slot 0): 128×112 2bpp tiles  |
//! | 0x011B2-0x011CF| State vector: 30 bytes, one per saved slot   |
//! | 0x011D5-0x011D6| State vector checksum (sum, xor)             |
//! | 0x02000-0x1FFFF| Photo slots 1-30 (4KB each, 2 per bank)     |
//!
//! ## State vector
//!
//! Each byte tracks whether a slot is occupied:
//! - `0xFF` = empty/erased
//! - `0x00..0x1D` = image number (occupied)
//!
//! References:
//! - https://gbdev.io/pandocs/Gameboy_Camera.html
//! - https://github.com/Raphael-Boichot/Inject-pictures-in-your-Game-Boy-Camera-saves
//! - https://github.com/untoxa/gb-photo/

use crate::log::{LogCategory, RateLimiter};
use crate::{log_info, log_info_limited};

pub(crate) const RAM_BANK_SIZE: usize = 0x2000; // 8KB

/// Start of the 30-byte state vector in SRAM bank 0.
const STATE_VECTOR_OFFSET: usize = 0x11B2;
const NUM_PHOTO_SLOTS: usize = 30;

/// Game Boy Camera sensor state, hardware registers, and photo storage.
///
/// Owns the 128KB cartridge RAM as well as all sensor-emulation fields.
/// Used exclusively by `PocketCamera` in the cartridge layer.
pub struct Camera {
    /// Hardware registers A000-A07F (when RAM bank >= 0x10).
    pub regs: [u8; 0x80],
    /// Raw 8-bit grayscale webcam image, 128×112 pixels (0=black, 255=white).
    pub image: Box<[u8; 128 * 112]>,
    pub image_ready: bool,
    pub capture_dirty: bool,
    /// Smoothed exposure factor — prevents autoexposure oscillation.
    pub exposure_smooth: f32,
    /// Optional override; when `Some`, bypasses ROM-controlled exposure.
    pub exposure_override: Option<u16>,
    /// 128KB cartridge RAM (16 × 8KB banks for photo storage).
    pub ram: Vec<u8>,
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            regs: [0; 0x80],
            image: Box::new([0; 128 * 112]),
            image_ready: false,
            capture_dirty: false,
            exposure_smooth: 1.0,
            exposure_override: None,
            ram: vec![0; 128 * 1024],
        }
    }

    /// Set camera image data from external source (e.g., webcam).
    /// Expects 128x112 pixels as raw 8-bit grayscale (0=black, 255=white).
    pub fn set_image(&mut self, data: &[u8]) {
        let len = data.len().min(128 * 112);
        self.image.copy_from_slice(&data[..len]);
        self.image_ready = true;

        static SET_IMAGE_LIMITER: RateLimiter = RateLimiter::new(30);
        if len > 0 {
            let sum: u32 = data[..len].iter().map(|&x| x as u32).sum();
            let avg = sum / len as u32;
            let min = data[..len].iter().copied().min().unwrap_or(0);
            let max = data[..len].iter().copied().max().unwrap_or(0);
            log_info_limited!(
                LogCategory::Camera,
                &SET_IMAGE_LIMITER,
                "set_camera_image: {} pixels, avg={} min={} max={}",
                len,
                avg,
                min,
                max
            );
        }
    }

    /// Read a camera hardware register (index 0x00-0x7F).
    #[inline]
    pub fn reg(&self, index: u8) -> u8 {
        self.regs[(index & 0x7F) as usize]
    }

    /// Set or clear the exposure override.
    pub fn set_exposure_override(&mut self, value: Option<u16>) {
        self.exposure_override = value;
    }

    #[inline]
    pub fn is_image_ready(&self) -> bool {
        self.image_ready
    }

    #[inline]
    pub fn is_capture_dirty(&self) -> bool {
        self.capture_dirty
    }

    #[inline]
    pub fn clear_capture_dirty(&mut self) {
        self.capture_dirty = false;
    }

    /// Get a reference to the raw SRAM for the active capture buffer (slot 0).
    /// Returns the 3,584-byte 2bpp tile region at offset 0x0100.
    pub fn capture_sram(&self) -> &[u8] {
        const PHOTO_BYTES: usize = 128 / 8 * 112 / 8 * 16; // 3584
        let end = (0x0100 + PHOTO_BYTES).min(self.ram.len());
        &self.ram[0x0100..end]
    }

    /// Process a camera capture: emulate M64282FP sensor and convert to Game Boy tiles.
    /// The Game Boy Camera stores captured images as tiles starting at SRAM offset 0x0100.
    /// Format: 16 tiles wide × 14 tiles tall = 224 tiles, 16 bytes each = 3584 bytes.
    ///
    /// Sensor registers used:
    /// - A001: N (negative), VH, Gain (bits 4-5: 00=highest gain, 11=lowest)
    /// - A002-A003: Exposure time (16-bit, higher = brighter)
    /// - A004: Edge enhancement (bits 4-6), O flag (bit 0)
    /// - A005: Voltage offset (darkness level)
    /// - A006-A035: Dithering matrix (48 bytes for 4x4x3 threshold values)
    pub fn process_capture(&mut self, invert: bool) {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 112;
        const TILE_SIZE: usize = 8;
        const TILES_X: usize = WIDTH / TILE_SIZE;
        const TILES_Y: usize = HEIGHT / TILE_SIZE;
        const SRAM_OFFSET: usize = 0x0100;

        let reg_a001 = self.regs[0x01];
        let exposure_low = self.regs[0x02];
        let exposure_high = self.regs[0x03];
        let reg_a004 = self.regs[0x04];
        let voltage_offset = self.regs[0x05];

        let exposure = self
            .exposure_override
            .unwrap_or(((exposure_high as u16) << 8) | (exposure_low as u16));
        let gain_bits = (reg_a001 >> 4) & 0x03;
        let edge_mode = (reg_a004 >> 4) & 0x07;
        let output_negative = (reg_a001 & 0x02) != 0;

        log_info!(
            LogCategory::Camera,
            "Sensor: exposure={}, gain_bits={}, edge={}, offset={}, neg={}, invert={}",
            exposure,
            gain_bits,
            edge_mode,
            voltage_offset,
            output_negative,
            invert
        );

        let mut dither_thresholds: [[u8; 3]; 16] = [[0; 3]; 16];
        for (i, row) in dither_thresholds.iter_mut().enumerate() {
            for (t, cell) in row.iter_mut().enumerate() {
                let reg_idx = 0x06 + i * 3 + t;
                if reg_idx < 0x36 {
                    *cell = self.regs[reg_idx];
                }
            }
        }

        let dither_active = dither_thresholds
            .iter()
            .any(|t| t[0] != 0 || t[1] != 0 || t[2] != 0);

        log_info!(
            LogCategory::Camera,
            "Dither active={}, thresholds[0]=[{:02X},{:02X},{:02X}], [8]=[{:02X},{:02X},{:02X}]",
            dither_active,
            dither_thresholds[0][0],
            dither_thresholds[0][1],
            dither_thresholds[0][2],
            dither_thresholds[8][0],
            dither_thresholds[8][1],
            dither_thresholds[8][2]
        );

        let img_sum: u32 = self.image.iter().map(|&x| x as u32).sum();
        let img_avg = img_sum / (WIDTH * HEIGHT) as u32;
        let img_min = self.image.iter().copied().min().unwrap_or(0);
        let img_max = self.image.iter().copied().max().unwrap_or(0);
        log_info!(
            LogCategory::Camera,
            "Input image: avg={}, min={}, max={}, ready={}",
            img_avg,
            img_min,
            img_max,
            self.image_ready
        );

        let target_factor = if exposure > 0 {
            (exposure as f32) / 4096.0
        } else {
            0.0
        };
        let exposure_factor = self.exposure_smooth * 0.5 + target_factor * 0.5;
        self.exposure_smooth = exposure_factor;

        let gain_factor = match gain_bits {
            0b00 => 2.0,
            0b01 => 1.5,
            0b10 => 1.0,
            0b11 => 0.75,
            _ => 1.0,
        };

        let offset_adjustment = (voltage_offset as f32) / 255.0 * 64.0;

        log_info!(
            LogCategory::Camera,
            "Effect params: exposure_f={:.2}, gain_f={:.2}, offset_adj={:.1}",
            exposure_factor,
            gain_factor,
            offset_adjustment
        );

        let mut processed: Box<[u8; WIDTH * HEIGHT]> = Box::new([0; WIDTH * HEIGHT]);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = y * WIDTH + x;
                let raw = self.image[idx] as f32;
                let exposed = raw * exposure_factor;
                let offset_applied = exposed - offset_adjustment;
                let centered = offset_applied - 128.0;
                let gained = centered * gain_factor + 128.0;
                processed[idx] = gained.clamp(0.0, 255.0) as u8;
            }
        }

        if edge_mode > 0 {
            let edge_strength = (edge_mode as f32) / 7.0;
            let mut edge_enhanced = processed.clone();

            for y in 1..HEIGHT - 1 {
                for x in 1..WIDTH - 1 {
                    let idx = y * WIDTH + x;
                    let center = processed[idx] as i32;
                    let neighbors = [
                        processed[idx - WIDTH] as i32,
                        processed[idx + WIDTH] as i32,
                        processed[idx - 1] as i32,
                        processed[idx + 1] as i32,
                    ];
                    let avg_neighbors: i32 = neighbors.iter().sum::<i32>() / 4;
                    let edge = center - avg_neighbors;
                    let enhanced = center + (edge as f32 * edge_strength * 2.0) as i32;
                    edge_enhanced[idx] = enhanced.clamp(0, 255) as u8;
                }
            }
            processed = edge_enhanced;
        }

        let mut quantized: Box<[u8; WIDTH * HEIGHT]> = Box::new([0; WIDTH * HEIGHT]);
        let mut color_counts = [0u32; 4];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = y * WIDTH + x;
                let pixel = processed[idx];
                let dither_idx = (y % 4) * 4 + (x % 4);
                let thresholds = &dither_thresholds[dither_idx];

                let color = if dither_active {
                    if pixel < thresholds[0] {
                        0
                    } else if pixel < thresholds[1] {
                        1
                    } else if pixel < thresholds[2] {
                        2
                    } else {
                        3
                    }
                } else {
                    let inverted = 255 - pixel;
                    (inverted / 64).min(3)
                };

                let final_color = if output_negative || invert { 3 - color } else { color };
                quantized[idx] = final_color;
                color_counts[final_color as usize] += 1;
            }
        }

        let proc_sum: u32 = processed.iter().map(|&x| x as u32).sum();
        let proc_avg = proc_sum / (WIDTH * HEIGHT) as u32;
        let proc_min = processed.iter().copied().min().unwrap_or(0);
        let proc_max = processed.iter().copied().max().unwrap_or(0);
        log_info!(
            LogCategory::Camera,
            "Processed: avg={}, min={}, max={}",
            proc_avg,
            proc_min,
            proc_max
        );
        log_info!(
            LogCategory::Camera,
            "Quantized: colors [0]={}, [1]={}, [2]={}, [3]={}",
            color_counts[0],
            color_counts[1],
            color_counts[2],
            color_counts[3]
        );

        for tile_y in 0..TILES_Y {
            for tile_x in 0..TILES_X {
                let tile_index = tile_y * TILES_X + tile_x;
                let sram_addr = SRAM_OFFSET + tile_index * 16;

                for row in 0..TILE_SIZE {
                    let pixel_y = tile_y * TILE_SIZE + row;
                    let mut low_byte: u8 = 0;
                    let mut high_byte: u8 = 0;

                    for col in 0..TILE_SIZE {
                        let pixel_x = tile_x * TILE_SIZE + col;
                        let pixel_index = pixel_y * WIDTH + pixel_x;
                        let color = quantized[pixel_index];
                        let bit_pos = 7 - col;
                        low_byte |= (color & 0x01) << bit_pos;
                        high_byte |= ((color >> 1) & 0x01) << bit_pos;
                    }

                    if sram_addr + row * 2 + 1 < self.ram.len() {
                        self.ram[sram_addr + row * 2] = low_byte;
                        self.ram[sram_addr + row * 2 + 1] = high_byte;
                    }
                }
            }
        }
    }

    /// Decode a GB Camera photo slot from SRAM into RGBA pixel data.
    /// Slot 0 = active capture buffer (bank 0, offset 0x0100).
    /// Slots 1-30 = saved photos in banks 1-15 (2 per bank).
    /// Returns 128×112×4 bytes of RGBA, or empty vec if slot is unoccupied.
    pub fn decode_photo(&self, slot: u8) -> Vec<u8> {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 112;
        const TILE_SIZE: usize = 8;
        const TILES_X: usize = WIDTH / TILE_SIZE;
        const TILES_Y: usize = HEIGHT / TILE_SIZE;
        const TILE_BYTES: usize = 16;
        const PHOTO_BYTES: usize = TILES_X * TILES_Y * TILE_BYTES; // 3584

        if (1..=30).contains(&slot) {
            let state_idx = STATE_VECTOR_OFFSET + (slot - 1) as usize;
            if state_idx < self.ram.len() && self.ram[state_idx] == 0xFF {
                return Vec::new();
            }
        }

        let sram_offset = if slot == 0 {
            0x0100
        } else {
            let adjusted = (slot - 1) as usize;
            let bank = adjusted / 2 + 1;
            let offset_in_bank = (adjusted % 2) * 0x1000;
            bank * RAM_BANK_SIZE + offset_in_bank
        };

        if sram_offset + PHOTO_BYTES > self.ram.len() {
            return Vec::new();
        }

        let palette: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];

        for tile_y in 0..TILES_Y {
            for tile_x in 0..TILES_X {
                let tile_index = tile_y * TILES_X + tile_x;
                let tile_offset = sram_offset + tile_index * TILE_BYTES;

                for row in 0..TILE_SIZE {
                    let low = self.ram[tile_offset + row * 2];
                    let high = self.ram[tile_offset + row * 2 + 1];

                    for col in 0..TILE_SIZE {
                        let bit = 7 - col;
                        let color_idx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);
                        let gray = palette[color_idx as usize];
                        let px = tile_x * TILE_SIZE + col;
                        let py = tile_y * TILE_SIZE + row;
                        let i = (py * WIDTH + px) * 4;
                        rgba[i] = gray;
                        rgba[i + 1] = gray;
                        rgba[i + 2] = gray;
                        rgba[i + 3] = 255;
                    }
                }
            }
        }

        rgba
    }

    /// Encode RGBA pixel data into a GB Camera SRAM slot (inverse of decode_photo).
    /// Accepts 128x112x4 RGBA bytes. Maps gray channel to 2-bit colors and packs into tiles.
    /// Also marks the slot as occupied in the state vector.
    pub fn encode_photo(&mut self, slot: u8, rgba: &[u8]) -> bool {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 112;
        const TILE_SIZE: usize = 8;
        const TILES_X: usize = WIDTH / TILE_SIZE;
        const TILES_Y: usize = HEIGHT / TILE_SIZE;
        const TILE_BYTES: usize = 16;
        const PHOTO_BYTES: usize = TILES_X * TILES_Y * TILE_BYTES;

        if slot == 0 || slot > 30 || rgba.len() != WIDTH * HEIGHT * 4 {
            return false;
        }

        let adjusted = (slot - 1) as usize;
        let bank = adjusted / 2 + 1;
        let offset_in_bank = (adjusted % 2) * 0x1000;
        let sram_offset = bank * RAM_BANK_SIZE + offset_in_bank;

        if sram_offset + PHOTO_BYTES > self.ram.len() {
            return false;
        }

        for tile_y in 0..TILES_Y {
            for tile_x in 0..TILES_X {
                let tile_index = tile_y * TILES_X + tile_x;
                let sram_addr = sram_offset + tile_index * TILE_BYTES;

                for row in 0..TILE_SIZE {
                    let pixel_y = tile_y * TILE_SIZE + row;
                    let mut low_byte: u8 = 0;
                    let mut high_byte: u8 = 0;

                    for col in 0..TILE_SIZE {
                        let pixel_x = tile_x * TILE_SIZE + col;
                        let i = (pixel_y * WIDTH + pixel_x) * 4;
                        let gray = rgba[i];
                        let color: u8 = match gray {
                            0xC0..=0xFF => 0,
                            0x80..=0xBF => 1,
                            0x40..=0x7F => 2,
                            0x00..=0x3F => 3,
                        };
                        let bit_pos = 7 - col;
                        low_byte |= (color & 0x01) << bit_pos;
                        high_byte |= ((color >> 1) & 0x01) << bit_pos;
                    }

                    self.ram[sram_addr + row * 2] = low_byte;
                    self.ram[sram_addr + row * 2 + 1] = high_byte;
                }
            }
        }

        self.set_state_vector_entry(slot, adjusted as u8);
        true
    }

    /// Clear a GB Camera SRAM slot (zero tile data and mark empty in state vector).
    pub fn clear_photo_slot(&mut self, slot: u8) {
        const PHOTO_BYTES: usize = (128 / 8) * (112 / 8) * 16; // 3584

        if slot == 0 || slot > 30 {
            return;
        }

        let adjusted = (slot - 1) as usize;
        let bank = adjusted / 2 + 1;
        let offset_in_bank = (adjusted % 2) * 0x1000;
        let sram_offset = bank * RAM_BANK_SIZE + offset_in_bank;

        if sram_offset + PHOTO_BYTES <= self.ram.len() {
            self.ram[sram_offset..sram_offset + PHOTO_BYTES].fill(0);
        }

        self.set_state_vector_entry(slot, 0xFF);
    }

    /// Derive the contrast level (0-15) from the current dither matrix registers.
    /// Returns 0-15 if matched against known gb-photo threshold tables, or -1 if unknown.
    pub fn contrast(&self) -> i32 {
        const HIGH_LIGHT: [[u8; 4]; 16] = [
            [0x80, 0x8F, 0xD0, 0xE6],
            [0x82, 0x90, 0xC8, 0xE3],
            [0x84, 0x90, 0xC0, 0xE0],
            [0x85, 0x91, 0xB8, 0xDD],
            [0x86, 0x91, 0xB1, 0xDB],
            [0x87, 0x92, 0xAA, 0xD8],
            [0x88, 0x92, 0xA5, 0xD5],
            [0x89, 0x92, 0xA2, 0xD2],
            [0x8A, 0x92, 0xA1, 0xC8],
            [0x8B, 0x92, 0xA0, 0xBE],
            [0x8C, 0x92, 0x9E, 0xB4],
            [0x8D, 0x92, 0x9C, 0xAC],
            [0x8E, 0x92, 0x9B, 0xA5],
            [0x8F, 0x92, 0x99, 0xA0],
            [0x90, 0x92, 0x97, 0x9A],
            [0x92, 0x92, 0x92, 0x92],
        ];
        const LOW_LIGHT: [[u8; 4]; 16] = [
            [0x80, 0x94, 0xDC, 0xFF],
            [0x82, 0x95, 0xD2, 0xFF],
            [0x84, 0x96, 0xCA, 0xFF],
            [0x86, 0x96, 0xC4, 0xFF],
            [0x88, 0x97, 0xBE, 0xFF],
            [0x8A, 0x97, 0xB8, 0xFF],
            [0x8B, 0x98, 0xB2, 0xF5],
            [0x8C, 0x98, 0xAC, 0xEB],
            [0x8D, 0x98, 0xAA, 0xDD],
            [0x8E, 0x98, 0xA8, 0xD0],
            [0x8F, 0x98, 0xA6, 0xC4],
            [0x90, 0x98, 0xA4, 0xBA],
            [0x92, 0x98, 0xA1, 0xB2],
            [0x94, 0x98, 0x9D, 0xA8],
            [0x96, 0x98, 0x99, 0xA0],
            [0x98, 0x98, 0x98, 0x98],
        ];

        let mut t = [0xFFu8; 3];
        for pos in 0..16 {
            for (ch, th) in t.iter_mut().enumerate() {
                let val = self.regs[0x06 + pos * 3 + ch];
                if val < *th {
                    *th = val;
                }
            }
        }

        for (level, row) in LOW_LIGHT.iter().enumerate() {
            if t[0] == row[0] && t[1] == row[1] && t[2] == row[2] {
                return level as i32;
            }
        }
        for (level, row) in HIGH_LIGHT.iter().enumerate() {
            if t[0] == row[0] && t[1] == row[1] && t[2] == row[2] {
                return level as i32;
            }
        }
        -1
    }

    /// Return the number of occupied photo slots (0-30) by scanning the state vector.
    pub fn photo_count(&self) -> u8 {
        let end = (STATE_VECTOR_OFFSET + NUM_PHOTO_SLOTS).min(self.ram.len());
        self.ram[STATE_VECTOR_OFFSET..end]
            .iter()
            .filter(|&&b| b != 0xFF)
            .count() as u8
    }

    fn set_state_vector_entry(&mut self, slot: u8, value: u8) {
        if slot == 0 || slot > NUM_PHOTO_SLOTS as u8 {
            return;
        }
        let idx = STATE_VECTOR_OFFSET + (slot - 1) as usize;
        if idx >= self.ram.len() {
            return;
        }
        self.ram[idx] = value;
        self.update_state_vector_checksum();
    }

    fn update_state_vector_checksum(&mut self) {
        const CHECKSUM_OFFSET: usize = 0x11D5;
        let end = STATE_VECTOR_OFFSET + NUM_PHOTO_SLOTS;
        if end > self.ram.len() || CHECKSUM_OFFSET + 1 >= self.ram.len() {
            return;
        }
        let mut sum: u8 = 0;
        let mut xor: u8 = 0;
        for &b in &self.ram[STATE_VECTOR_OFFSET..end] {
            sum = sum.wrapping_add(b);
            xor ^= b;
        }
        self.ram[CHECKSUM_OFFSET] = sum;
        self.ram[CHECKSUM_OFFSET + 1] = xor;
    }
}
