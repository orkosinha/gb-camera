//! Game Boy Camera (Pocket Camera) sensor emulation and photo decoding.

use super::{Memory, RAM_BANK_SIZE};
use crate::log::{LogCategory, RateLimiter};
use crate::{log_info, log_info_limited};

impl Memory {
    /// Set camera image data from external source (e.g., webcam).
    /// Expects 128x112 pixels as raw 8-bit grayscale (0=black, 255=white).
    /// The sensor emulation will process this during capture.
    pub fn set_camera_image(&mut self, data: &[u8]) {
        let len = data.len().min(128 * 112);

        // Store raw 8-bit grayscale values
        self.camera_image.copy_from_slice(&data[..len]);
        self.camera_image_ready = true;

        // Log occasionally to verify data is being received
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
    pub(super) fn process_camera_capture(&mut self, invert: bool) {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 112;
        const TILE_SIZE: usize = 8;
        const TILES_X: usize = WIDTH / TILE_SIZE;
        const TILES_Y: usize = HEIGHT / TILE_SIZE;
        const SRAM_OFFSET: usize = 0x0100;

        // Read sensor registers
        let reg_a001 = self.camera_regs[0x01];
        let exposure_low = self.camera_regs[0x02];
        let exposure_high = self.camera_regs[0x03];
        let reg_a004 = self.camera_regs[0x04];
        let voltage_offset = self.camera_regs[0x05];

        // Parse register values
        let exposure = ((exposure_high as u16) << 8) | (exposure_low as u16);
        let gain_bits = (reg_a001 >> 4) & 0x03; // 00=high gain, 11=low gain
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

        // Read dithering matrix from A006-A035 (48 bytes)
        // This encodes 3 threshold levels for a 4x4 pattern
        let mut dither_thresholds: [[u8; 3]; 16] = [[0; 3]; 16];
        for (i, row) in dither_thresholds.iter_mut().enumerate() {
            for (t, cell) in row.iter_mut().enumerate() {
                let reg_idx = 0x06 + i * 3 + t;
                if reg_idx < 0x36 {
                    *cell = self.camera_regs[reg_idx];
                }
            }
        }

        // Check if dither matrix was set (non-zero values)
        let dither_active = dither_thresholds
            .iter()
            .any(|t| t[0] != 0 || t[1] != 0 || t[2] != 0);

        // Log first few dither values for debugging
        // Log more dither threshold info for debugging
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

        // Log input image stats
        let img_sum: u32 = self.camera_image.iter().map(|&x| x as u32).sum();
        let img_avg = img_sum / (WIDTH * HEIGHT) as u32;
        let img_min = self.camera_image.iter().copied().min().unwrap_or(0);
        let img_max = self.camera_image.iter().copied().max().unwrap_or(0);
        log_info!(
            LogCategory::Camera,
            "Input image: avg={}, min={}, max={}, ready={}",
            img_avg,
            img_min,
            img_max,
            self.camera_image_ready
        );

        // Smooth exposure adjustment
        let target_factor = if exposure > 0 {
            (exposure as f32) / 4096.0
        } else {
            0.0
        };
        let exposure_factor = self.camera_exposure_smooth * 0.5 + target_factor * 0.5;
        self.camera_exposure_smooth = exposure_factor;

        // Calculate contrast multiplier from gain (higher gain = more contrast)
        let gain_factor = match gain_bits {
            0b00 => 2.0, // Highest gain
            0b01 => 1.5,
            0b10 => 1.0,
            0b11 => 0.75, // Lowest gain
            _ => 1.0,
        };

        // Voltage offset affects black level (higher = darker overall)
        let offset_adjustment = (voltage_offset as f32) / 255.0 * 64.0;

        log_info!(
            LogCategory::Camera,
            "Effect params: exposure_f={:.2}, gain_f={:.2}, offset_adj={:.1}",
            exposure_factor,
            gain_factor,
            offset_adjustment
        );

        // Process the image with sensor emulation
        let mut processed: Box<[u8; WIDTH * HEIGHT]> = Box::new([0; WIDTH * HEIGHT]);

        // First pass: apply exposure, offset, and gain (accurate sensor emulation)
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = y * WIDTH + x;
                let raw = self.camera_image[idx] as f32;

                // Apply exposure (brightness scaling)
                let exposed = raw * exposure_factor;

                // Apply voltage offset (shift black level)
                let offset_applied = exposed - offset_adjustment;

                // Apply gain (contrast around midpoint)
                let centered = offset_applied - 128.0;
                let gained = centered * gain_factor + 128.0;

                // Clamp to valid range
                processed[idx] = gained.clamp(0.0, 255.0) as u8;
            }
        }

        // Second pass: apply edge enhancement if enabled
        if edge_mode > 0 {
            let edge_strength = (edge_mode as f32) / 7.0;
            let mut edge_enhanced = processed.clone();

            for y in 1..HEIGHT - 1 {
                for x in 1..WIDTH - 1 {
                    let idx = y * WIDTH + x;

                    // Simple edge detection kernel (Laplacian-like)
                    let center = processed[idx] as i32;
                    let neighbors = [
                        processed[idx - WIDTH] as i32, // top
                        processed[idx + WIDTH] as i32, // bottom
                        processed[idx - 1] as i32,     // left
                        processed[idx + 1] as i32,     // right
                    ];
                    let avg_neighbors: i32 = neighbors.iter().sum::<i32>() / 4;
                    let edge = center - avg_neighbors;

                    // Enhance edges
                    let enhanced = center + (edge as f32 * edge_strength * 2.0) as i32;
                    edge_enhanced[idx] = enhanced.clamp(0, 255) as u8;
                }
            }
            processed = edge_enhanced;
        }

        // Third pass: quantize to 2-bit using dithering matrix
        let mut quantized: Box<[u8; WIDTH * HEIGHT]> = Box::new([0; WIDTH * HEIGHT]);
        let mut color_counts = [0u32; 4];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = y * WIDTH + x;
                let pixel = processed[idx];

                // Get dither position in 4x4 matrix
                let dither_idx = (y % 4) * 4 + (x % 4);
                let thresholds = &dither_thresholds[dither_idx];

                // Quantize using thresholds from dither matrix
                let color = if dither_active {
                    // Use dithering matrix: compare pixel against 3 thresholds
                    // Thresholds define boundaries between colors 0-1, 1-2, 2-3
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
                    // Fallback: simple linear quantization when no dither matrix
                    // Invert because webcam 0=black but GB color 0=white
                    let inverted = 255 - pixel;
                    (inverted / 64).min(3)
                };

                // Apply output negative flag if set
                let final_color = if output_negative || invert {
                    3 - color
                } else {
                    color
                };

                quantized[idx] = final_color;
                color_counts[final_color as usize] += 1;
            }
        }

        // Log processed pixel stats
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

        // Convert quantized image to tiles in SRAM
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

                    if sram_addr + row * 2 + 1 < self.cartridge_ram.len() {
                        self.cartridge_ram[sram_addr + row * 2] = low_byte;
                        self.cartridge_ram[sram_addr + row * 2 + 1] = high_byte;
                    }
                }
            }
        }
    }

    /// Decode a GB Camera photo slot from SRAM into RGBA pixel data.
    /// Slot 0 = active capture buffer (bank 0, offset 0x0100).
    /// Slots 1-30 = saved photos in banks 1-15 (2 per bank).
    /// Returns 128×112×4 bytes of RGBA, or empty vec if slot is unoccupied.
    pub fn decode_camera_photo(&self, slot: u8) -> Vec<u8> {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 112;
        const TILE_SIZE: usize = 8;
        const TILES_X: usize = WIDTH / TILE_SIZE;
        const TILES_Y: usize = HEIGHT / TILE_SIZE;
        const TILE_BYTES: usize = 16;
        const PHOTO_BYTES: usize = TILES_X * TILES_Y * TILE_BYTES; // 3584
        // GB Camera state vector: 30 bytes at SRAM 0x11B2, one per slot.
        // 0xFF = empty/erased, anything else = occupied.
        const STATE_VECTOR_OFFSET: usize = 0x11B2;

        // For saved slots (1-30), check the ROM's state vector
        if (1..=30).contains(&slot) {
            let state_idx = STATE_VECTOR_OFFSET + (slot - 1) as usize;
            if state_idx < self.cartridge_ram.len() && self.cartridge_ram[state_idx] == 0xFF {
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

        if sram_offset + PHOTO_BYTES > self.cartridge_ram.len() {
            return Vec::new();
        }

        let palette: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];

        for tile_y in 0..TILES_Y {
            for tile_x in 0..TILES_X {
                let tile_index = tile_y * TILES_X + tile_x;
                let tile_offset = sram_offset + tile_index * TILE_BYTES;

                for row in 0..TILE_SIZE {
                    let low = self.cartridge_ram[tile_offset + row * 2];
                    let high = self.cartridge_ram[tile_offset + row * 2 + 1];

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

    /// Derive the contrast level (0-15) from the current dither matrix in camera registers.
    /// Returns 0-15 if matched against known gb-photo threshold tables, or -1 if unknown.
    ///
    /// The dither matrix (regs A006-A035) encodes 16 positions × 3 thresholds each.
    /// For any dither pattern, the minimum of each threshold channel across all 16
    /// positions equals the base threshold for that channel. We match [t0, t1, t2]
    /// against the standard high-light and low-light tables.
    pub fn camera_contrast(&self) -> i32 {
        // High-light dither thresholds (16 contrast levels × 4 thresholds)
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
        // Low-light dither thresholds
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

        // Extract minimum of each threshold channel across all 16 dither positions.
        // This yields the base thresholds [t0, t1, t2].
        let mut t = [0xFFu8; 3];
        for pos in 0..16 {
            for ch in 0..3 {
                let val = self.camera_regs[0x06 + pos * 3 + ch];
                if val < t[ch] {
                    t[ch] = val;
                }
            }
        }

        // Match against known tables (check first 3 of 4 thresholds)
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

    /// Check if camera image is ready.
    pub fn is_camera_image_ready(&self) -> bool {
        self.camera_image_ready
    }

    /// Check if the active capture buffer has changed since last clear.
    pub fn is_camera_capture_dirty(&self) -> bool {
        self.camera_capture_dirty
    }

    /// Clear the capture dirty flag.
    pub fn clear_camera_capture_dirty(&mut self) {
        self.camera_capture_dirty = false;
    }

    /// Get a reference to the raw SRAM for the active capture buffer (slot 0).
    /// Returns the 3,584-byte 2bpp tile region at offset 0x0100.
    pub fn camera_capture_sram(&self) -> &[u8] {
        const PHOTO_BYTES: usize = 128 / 8 * 112 / 8 * 16; // 3584
        let end = (0x0100 + PHOTO_BYTES).min(self.cartridge_ram.len());
        &self.cartridge_ram[0x0100..end]
    }
}
