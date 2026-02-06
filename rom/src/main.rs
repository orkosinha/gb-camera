//! Minimal Game Boy Camera test ROM generator.
//!
//! Generates a ROM that continuously captures camera images to SRAM.
//! Uses authentic dithering patterns and contrast settings from gb-photo.
//!
//! The captured image data is stored at SRAM offset 0x0100 (address 0xA100).
//! Format: 128x112 pixels as 2bpp tiles (16x14 tiles, 3584 bytes).

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

const ROM_SIZE: usize = 32768; // 32KB minimum

/// Nintendo logo - required for boot ROM validation
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

// ============================================================================
// Dither patterns from gb-photo (4x4 matrices, values 0x00-0x0F)
// https://github.com/untoxa/gb-photo/
// These are indices that determine ordering of thresholds
// ============================================================================

/// Standard ordered dither (Bayer-like)
const PATTERN_STANDARD: [u8; 16] = [
    0x00, 0x0C, 0x03, 0x0F, 0x08, 0x04, 0x0B, 0x07, 0x02, 0x0E, 0x01, 0x0D, 0x0A, 0x06, 0x09, 0x05,
];

/// 2x2 block pattern
const PATTERN_2X2: [u8; 16] = [
    0x01, 0x01, 0x0A, 0x0A, 0x01, 0x01, 0x0A, 0x0A, 0x0D, 0x0D, 0x03, 0x03, 0x0D, 0x0D, 0x03, 0x03,
];

/// Grid pattern
const PATTERN_GRID: [u8; 16] = [
    0x0C, 0x08, 0x07, 0x0C, 0x07, 0x01, 0x02, 0x07, 0x07, 0x07, 0x02, 0x08, 0x0D, 0x08, 0x08, 0x0C,
];

/// Maze pattern
const PATTERN_MAZE: [u8; 16] = [
    0x00, 0x01, 0x03, 0x05, 0x02, 0x0A, 0x0B, 0x0D, 0x04, 0x0C, 0x07, 0x08, 0x06, 0x0E, 0x09, 0x0F,
];

/// Nest pattern
const PATTERN_NEST: [u8; 16] = [
    0x00, 0x01, 0x08, 0x0B, 0x02, 0x06, 0x0A, 0x0C, 0x09, 0x0E, 0x03, 0x04, 0x0D, 0x0F, 0x05, 0x07,
];

/// Fuzz pattern
const PATTERN_FUZZ: [u8; 16] = [
    0x00, 0x09, 0x0E, 0x07, 0x04, 0x0D, 0x02, 0x0B, 0x08, 0x01, 0x06, 0x0F, 0x0C, 0x05, 0x0A, 0x03,
];

/// Vertical stripes
const PATTERN_VERTICAL: [u8; 16] = [
    0x00, 0x0A, 0x07, 0x0D, 0x01, 0x0B, 0x04, 0x0E, 0x02, 0x08, 0x05, 0x0F, 0x03, 0x09, 0x06, 0x0C,
];

/// Horizontal stripes
const PATTERN_HORIZONTAL: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x0A, 0x0B, 0x08, 0x09, 0x07, 0x04, 0x05, 0x06, 0x0D, 0x0E, 0x0F, 0x0C,
];

/// Diagonal pattern
const PATTERN_DIAGONAL: [u8; 16] = [
    0x00, 0x08, 0x04, 0x0C, 0x0D, 0x01, 0x09, 0x05, 0x06, 0x0E, 0x02, 0x0A, 0x0B, 0x07, 0x0F, 0x03,
];

// ============================================================================
// Threshold lookup tables from gb-photo
// Each row is [threshold0, threshold1, threshold2, threshold3] for a contrast level
// ============================================================================

/// High-light dither thresholds (16 contrast levels × 4 thresholds)
const DITHER_HIGH_LIGHT: [[u8; 4]; 16] = [
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

/// Low-light dither thresholds (16 contrast levels × 4 thresholds)
const DITHER_LOW_LIGHT: [[u8; 4]; 16] = [
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

#[derive(Clone, Copy, Debug)]
enum DitherPattern {
    Standard,
    Block2x2,
    Grid,
    Maze,
    Nest,
    Fuzz,
    Vertical,
    Horizontal,
    Diagonal,
}

impl DitherPattern {
    fn data(&self) -> &'static [u8; 16] {
        match self {
            DitherPattern::Standard => &PATTERN_STANDARD,
            DitherPattern::Block2x2 => &PATTERN_2X2,
            DitherPattern::Grid => &PATTERN_GRID,
            DitherPattern::Maze => &PATTERN_MAZE,
            DitherPattern::Nest => &PATTERN_NEST,
            DitherPattern::Fuzz => &PATTERN_FUZZ,
            DitherPattern::Vertical => &PATTERN_VERTICAL,
            DitherPattern::Horizontal => &PATTERN_HORIZONTAL,
            DitherPattern::Diagonal => &PATTERN_DIAGONAL,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            DitherPattern::Standard => "standard",
            DitherPattern::Block2x2 => "2x2",
            DitherPattern::Grid => "grid",
            DitherPattern::Maze => "maze",
            DitherPattern::Nest => "nest",
            DitherPattern::Fuzz => "fuzz",
            DitherPattern::Vertical => "vertical",
            DitherPattern::Horizontal => "horizontal",
            DitherPattern::Diagonal => "diagonal",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "standard" | "default" | "bayer" => Some(DitherPattern::Standard),
            "2x2" | "block" => Some(DitherPattern::Block2x2),
            "grid" => Some(DitherPattern::Grid),
            "maze" => Some(DitherPattern::Maze),
            "nest" => Some(DitherPattern::Nest),
            "fuzz" => Some(DitherPattern::Fuzz),
            "vertical" | "vert" => Some(DitherPattern::Vertical),
            "horizontal" | "horiz" => Some(DitherPattern::Horizontal),
            "diagonal" | "diag" => Some(DitherPattern::Diagonal),
            _ => None,
        }
    }
}

/// Camera configuration
struct CameraConfig {
    pattern: DitherPattern,
    contrast: u8,       // 0-15
    high_light: bool,   // true = high light, false = low light
    exposure: u16,      // 0x0000-0xFFFF
    gain: u8,           // 0-3 (0=highest, 3=lowest)
    edge_enhance: u8,   // 0-7
    voltage_offset: u8, // 0-255
    invert: bool,
    release: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            pattern: DitherPattern::Standard,
            contrast: 9,          // Middle contrast
            high_light: false,    // Low light mode (wider dynamic range)
            exposure: 0x1000,     // Normal exposure
            gain: 0,              // Highest gain
            edge_enhance: 0,      // No edge enhancement
            voltage_offset: 0x80, // Middle offset
            invert: false,
            release: false,
        }
    }
}

impl std::fmt::Display for CameraConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Camera ROM Configuration:")?;
        writeln!(f, "  Pattern:    {}", self.pattern.name())?;
        writeln!(f, "  Contrast:   {}/15", self.contrast)?;
        writeln!(
            f,
            "  Light mode: {}",
            if self.high_light { "high" } else { "low" }
        )?;
        writeln!(f, "  Exposure:   0x{:04X}", self.exposure)?;
        writeln!(
            f,
            "  Gain:       {} ({})",
            self.gain,
            match self.gain {
                0 => "highest",
                1 => "high",
                2 => "low",
                _ => "lowest",
            }
        )?;
        writeln!(f, "  Edge:       {}", self.edge_enhance)?;
        writeln!(f, "  Offset:     {}", self.voltage_offset)?;
        writeln!(f, "  Invert:     {}", self.invert)
    }
}

/// Generate the 48-byte dither matrix for camera registers A006-A035
fn generate_dither_matrix(pattern: &[u8; 16], high_light: bool, contrast: u8) -> [u8; 48] {
    let thresholds = if high_light {
        &DITHER_HIGH_LIGHT[contrast as usize]
    } else {
        &DITHER_LOW_LIGHT[contrast as usize]
    };

    let mut matrix = [0u8; 48];

    // The dither matrix has 16 positions (4x4), each with 3 threshold values
    // These thresholds define boundaries between the 4 grayscale levels
    for pos in 0..16 {
        let idx = pattern[pos] as usize;
        // Interpolate thresholds based on pattern index
        // idx 0 = darkest thresholds, idx 15 = brightest
        let t0 = interpolate_threshold(thresholds[0], thresholds[1], idx);
        let t1 = interpolate_threshold(thresholds[1], thresholds[2], idx);
        let t2 = interpolate_threshold(thresholds[2], thresholds[3], idx);

        matrix[pos * 3] = t0;
        matrix[pos * 3 + 1] = t1;
        matrix[pos * 3 + 2] = t2;
    }

    matrix
}

fn interpolate_threshold(low: u8, high: u8, idx: usize) -> u8 {
    let range = (high as i32) - (low as i32);
    let offset = (range * idx as i32) / 16;
    (low as i32 + offset).clamp(0, 255) as u8
}

fn build_rom(config: &CameraConfig) -> Vec<u8> {
    let mut rom = vec![0u8; ROM_SIZE];

    // Entry point at 0x0100: NOP, JP $0150
    rom[0x100] = 0x00; // NOP
    rom[0x101] = 0xC3; // JP nn
    rom[0x102] = 0x50; // low byte of 0x0150
    rom[0x103] = 0x01; // high byte of 0x0150

    // Nintendo logo at 0x0104-0x0133
    rom[0x104..0x134].copy_from_slice(&NINTENDO_LOGO);

    // Title at 0x0134-0x0143: "CAMTEST"
    let title = b"CAMTEST";
    rom[0x134..0x134 + title.len()].copy_from_slice(title);

    // Cartridge type at 0x0147: 0xFC = Pocket Camera
    rom[0x147] = 0xFC;

    // ROM size at 0x0148: 0x00 = 32KB (2 banks)
    rom[0x148] = 0x00;

    // RAM size at 0x0149: 0x04 = 128KB (16 banks)
    rom[0x149] = 0x04;

    // Header checksum at 0x014D
    let checksum: u8 = rom[0x134..=0x14C]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_sub(b).wrapping_sub(1));
    rom[0x14D] = checksum;

    // Pre-compute all 16 dither matrices at 0x0300-0x05FF (16 x 48 = 768 bytes)
    let pattern_data = config.pattern.data();
    for level in 0..16u8 {
        let matrix = generate_dither_matrix(pattern_data, config.high_light, level);
        let offset = 0x1000 + (level as usize) * 48;
        rom[offset..offset + 48].copy_from_slice(&matrix);
    }

    // Font data at 0x0600: 7 characters for "gb-film", 16 bytes each (2bpp)
    // 1bpp masks (1 = white pixel on black background)
    let font_masks: [[u8; 8]; 7] = [
        [0x00, 0x70, 0x88, 0x88, 0x78, 0x08, 0x88, 0x70], // g
        [0x80, 0x80, 0xF0, 0x88, 0x88, 0x88, 0xF0, 0x00], // b
        [0x00, 0x00, 0x00, 0x00, 0xF8, 0x00, 0x00, 0x00], // -
        [0x30, 0x40, 0xF0, 0x40, 0x40, 0x40, 0x40, 0x00], // f
        [0x40, 0x00, 0x40, 0x40, 0x40, 0x40, 0x40, 0x00], // i
        [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x30, 0x00], // l
        [0x00, 0x00, 0x6C, 0x92, 0x92, 0x92, 0x92, 0x00], // m
    ];
    for (i, masks) in font_masks.iter().enumerate() {
        let offset = 0x0600 + i * 16;
        for (row, &mask) in masks.iter().enumerate() {
            let inv = !mask;
            rom[offset + row * 2] = inv; // low bitplane
            rom[offset + row * 2 + 1] = inv; // high bitplane
        }
    }

    // Digit font data at 0x0670: 10 digits (0-9), 16 bytes each (2bpp)
    // 1bpp masks (5x7 pixel bitmaps, 1 = white pixel on black background)
    let digit_masks: [[u8; 8]; 10] = [
        [0x70, 0x88, 0x98, 0xA8, 0xC8, 0x88, 0x70, 0x00], // 0
        [0x20, 0x60, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00], // 1
        [0x70, 0x88, 0x08, 0x30, 0x40, 0x80, 0xF8, 0x00], // 2
        [0x70, 0x88, 0x08, 0x30, 0x08, 0x88, 0x70, 0x00], // 3
        [0x10, 0x30, 0x50, 0x90, 0xF8, 0x10, 0x10, 0x00], // 4
        [0xF8, 0x80, 0xF0, 0x08, 0x08, 0x88, 0x70, 0x00], // 5
        [0x30, 0x40, 0x80, 0xF0, 0x88, 0x88, 0x70, 0x00], // 6
        [0xF8, 0x08, 0x10, 0x20, 0x40, 0x40, 0x40, 0x00], // 7
        [0x70, 0x88, 0x88, 0x70, 0x88, 0x88, 0x70, 0x00], // 8
        [0x70, 0x88, 0x88, 0x78, 0x08, 0x10, 0x60, 0x00], // 9
    ];
    for (i, masks) in digit_masks.iter().enumerate() {
        let offset = 0x0670 + i * 16;
        for (row, &mask) in masks.iter().enumerate() {
            let inv = !mask;
            rom[offset + row * 2] = inv; // low bitplane
            rom[offset + row * 2 + 1] = inv; // high bitplane
        }
    }

    // Build A001 register value: N (bit 1), VH (bit 0), Gain (bits 4-5)
    let reg_a001 = (config.gain & 0x03) << 4 | if config.invert { 0x00 } else { 0x02 };

    // Build A004 register value: Edge (bits 4-6), O flag (bit 0)
    let reg_a004 = (config.edge_enhance & 0x07) << 4;

    // Machine code starting at 0x0150
    //
    // HRAM variables:
    //   FF80: exposure low byte
    //   FF81: exposure high byte
    //   FF82: contrast level (0-15)
    //
    // D-PAD controls (read each frame):
    //   Up:    increase brightness (exposure += 0x0400)
    //   Down:  decrease brightness (exposure -= 0x0400)
    //   Right: increase contrast (+1, max 15)
    //   Left:  decrease contrast (-1, min 0)
    let mut code: Vec<u8> = Vec::new();

    // === INIT (0x0150) ===
    code.extend_from_slice(&[
        // Enable SRAM
        0x3E,
        0x0A, // ld a, $0A
        0xEA,
        0x00,
        0x00, // ld [$0000], a
        // Store default exposure low byte in HRAM
        0x3E,
        (config.exposure & 0xFF) as u8, // ld a, exp_lo
        0xE0,
        0x80, // ldh [$FF80], a
        // Store default exposure high byte in HRAM
        0x3E,
        (config.exposure >> 8) as u8, // ld a, exp_hi
        0xE0,
        0x81, // ldh [$FF81], a
        // Store default contrast level in HRAM
        0x3E,
        config.contrast, // ld a, contrast
        0xE0,
        0x82, // ldh [$FF82], a
    ]);

    // === Init new HRAM vars ===
    code.extend_from_slice(&[
        // FF84 = 0 (previous A-button state)
        0xAF,       // xor a
        0xE0, 0x84, // ldh [$FF84], a
        // FF85 = 30 (remaining slots)
        0x3E, 30,   // ld a, 30
        0xE0, 0x85, // ldh [$FF85], a
        // FF86 = 1 (next save slot)
        0x3E, 0x01, // ld a, 1
        0xE0, 0x86, // ldh [$FF86], a
    ]);

    // === Select SRAM bank 0 and init state vector ===
    code.extend_from_slice(&[
        // Select bank 0
        0xAF,             // xor a
        0xEA, 0x00, 0x40, // ld [$4000], a
        // Write 0xFF to 30 bytes at $B1B2 (state vector)
        0x21, 0xB2, 0xB1, // ld hl, $B1B2
        0x06, 30,         // ld b, 30
        0x3E, 0xFF,       // ld a, $FF
        // state_init_loop:
        0x22,             // ld [hl+], a
        0x05,             // dec b
        0x20, 0xFC,       // jr nz, state_init_loop (-4)
    ]);

    // === LCD INIT ===
    code.extend_from_slice(&[
        // BGP palette: standard grayscale mapping
        0x3E, 0xE4, // ld a, $E4
        0xE0, 0x47, // ldh [$FF47], a
        // Scroll registers: SCY=0, SCX=0
        0xAF, // xor a
        0xE0, 0x42, // ldh [$FF42], a
        0xE0, 0x43, // ldh [$FF43], a
    ]);

    // Fill all VRAM tile data ($8000-$8FFF) with $FF so everything starts black
    code.extend_from_slice(&[
        0x21, 0x00, 0x80, // ld hl, $8000
        0x01, 0x00, 0x10, // ld bc, $1000 (4096 bytes = 256 tiles)
        // fill_vram_loop:
        0x3E, 0xFF, // ld a, $FF
        0x22, // ld [hl+], a
        0x0B, // dec bc
        0x78, // ld a, b
        0xB1, // or c
        0x20, 0xF8, // jr nz, fill_vram_loop (-8)
    ]);

    // Fill tile map at $9800 with tile index $E0 (border tile, all black)
    code.extend_from_slice(&[
        0x21, 0x00, 0x98, // ld hl, $9800
        0x01, 0x00, 0x04, // ld bc, $0400 (1024 bytes)
        // fill_map_loop:
        0x3E, 0xE0, // ld a, $E0
        0x22, // ld [hl+], a
        0x0B, // dec bc
        0x78, // ld a, b
        0xB1, // or c
        0x20, 0xF8, // jr nz, fill_map_loop (-8)
    ]);

    // Copy font tile data from ROM $0600 to VRAM $8E10 (tiles 225-231, 112 bytes)
    code.extend_from_slice(&[
        0x21, 0x00, 0x06, // ld hl, $0600 (ROM source)
        0x11, 0x10, 0x8E, // ld de, $8E10 (VRAM dest, tile 225)
        0x06, 0x70, // ld b, 112
        // font_copy_loop:
        0x2A, // ld a, [hl+]
        0x12, // ld [de], a
        0x13, // inc de
        0x05, // dec b
        0x20, 0xFA, // jr nz, font_copy_loop (-6)
    ]);

    // Copy digit font data from ROM $0670 to VRAM $8E80 (tiles 232-241, 160 bytes)
    code.extend_from_slice(&[
        0x21, 0x70, 0x06, // ld hl, $0670 (ROM source)
        0x11, 0x80, 0x8E, // ld de, $8E80 (VRAM dest, tile 232)
        0x01, 0xA0, 0x00, // ld bc, $00A0 (160 bytes)
        // digit_copy_loop:
        0x2A,             // ld a, [hl+]
        0x12,             // ld [de], a
        0x13,             // inc de
        0x0B,             // dec bc
        0x78,             // ld a, b
        0xB1,             // or c
        0x20, 0xF8,       // jr nz, digit_copy_loop (-8)
    ]);

    // Write camera tile indices (0-223) into 16x14 region of 32-wide tile map
    // Centered: start at row 2, col 2 = $9800 + 2*32 + 2 = $9842
    // Use HRAM $FF83 for tile index, B = row counter, C = column counter
    code.extend_from_slice(&[
        0x21, 0x42, 0x98, // ld hl, $9842
        0xAF, // xor a (tile index = 0)
        0xE0, 0x83, // ldh [$FF83], a
        0x06, 0x0E, // ld b, 14 (row count)
        // row_loop:
        0xC5, // push bc
        0x0E, 0x10, // ld c, 16 (column count)
        // col_loop:
        0xF0, 0x83, // ldh a, [$FF83]
        0x22, // ld [hl+], a
        0x3C, // inc a
        0xE0, 0x83, // ldh [$FF83], a
        0x0D, // dec c
        0x20, 0xF7, // jr nz, col_loop (-9)
        // Advance HL by 16 to skip unused columns in 32-wide map
        0x11, 0x10, 0x00, // ld de, $0010
        0x19, // add hl, de
        0xC1, // pop bc
        0x05, // dec b
        0x20, 0xEC, // jr nz, row_loop (-20)
    ]);

    // Write "gb-film" tile indices at row 17, col 7 (centered in bottom border)
    // Row 17, col 7 = $9800 + 17*32 + 7 = $9A27
    code.extend_from_slice(&[
        0x21, 0x27, 0x9A, // ld hl, $9A27
        0x3E, 0xE1, // ld a, $E1 (first font tile)
        0x06, 0x07, // ld b, 7
        // text_loop:
        0x22, // ld [hl+], a
        0x3C, // inc a
        0x05, // dec b
        0x20, 0xFB, // jr nz, text_loop (-5)
    ]);

    // Write initial counter "30" at row 17, col 17 ($9800 + 17*32 + 17 = $9A31)
    // Digit tiles: 0=$E8, 1=$E9, ..., 9=$F1
    // '3' = $E8+3 = $EB, '0' = $E8+0 = $E8
    code.extend_from_slice(&[
        0x21, 0x31, 0x9A, // ld hl, $9A31
        0x3E, 0xEB,       // ld a, $EB ('3')
        0x22,             // ld [hl+], a
        0x3E, 0xE8,       // ld a, $E8 ('0')
        0x77,             // ld [hl], a
    ]);

    // Enable LCD: BG on, tile data at $8000, map at $9800
    code.extend_from_slice(&[
        0x3E, 0x91, // ld a, $91
        0xE0, 0x40, // ldh [$FF40], a
    ]);

    // Compute CAPTURE_LOOP address
    let capture_loop_addr = 0x0150 + code.len() as u16;

    // === CAPTURE_LOOP ===
    code.extend_from_slice(&[
        // Read joypad: select D-PAD (P14=0, P15=1)
        0x3E,
        0x20, // ld a, $20
        0xE0,
        0x00, // ldh [$FF00], a
        0xF0,
        0x00, // ldh a, [$FF00] (settle)
        0xF0,
        0x00, // ldh a, [$FF00] (read)
        0x2F, // cpl (invert: 1=pressed)
        0xE6,
        0x0F, // and $0F (mask d-pad bits)
        0x47, // ld b, a
        // --- Check Up (bit 2): increase exposure high byte by 4 ---
        0xCB,
        0x50, // bit 2, b
        0x28,
        0x0A, // jr z, +10 (skip to no_up)
        0xF0,
        0x81, // ldh a, [$FF81]
        0xFE,
        0xFC, // cp $FC
        0x30,
        0x04, // jr nc, +4 (already >= $FC, skip)
        0xC6,
        0x04, // add a, $04
        0xE0,
        0x81, // ldh [$FF81], a
        // --- Check Down (bit 3): decrease exposure high byte by 4 ---
        0xCB,
        0x58, // bit 3, b
        0x28,
        0x0A, // jr z, +10 (skip to no_down)
        0xF0,
        0x81, // ldh a, [$FF81]
        0xFE,
        0x04, // cp $04
        0x38,
        0x04, // jr c, +4 (already < $04, skip)
        0xD6,
        0x04, // sub $04
        0xE0,
        0x81, // ldh [$FF81], a
        // --- Check Right (bit 0): increase contrast ---
        0xCB,
        0x40, // bit 0, b
        0x28,
        0x09, // jr z, +9 (skip to no_right)
        0xF0,
        0x82, // ldh a, [$FF82]
        0xFE,
        0x0F, // cp $0F
        0x30,
        0x03, // jr nc, +3 (already at max, skip)
        0x3C, // inc a
        0xE0,
        0x82, // ldh [$FF82], a
        // --- Check Left (bit 1): decrease contrast ---
        0xCB,
        0x48, // bit 1, b
        0x28,
        0x08, // jr z, +8 (skip to no_left)
        0xF0,
        0x82, // ldh a, [$FF82]
        0xB7, // or a
        0x28,
        0x03, // jr z, +3 (already at 0, skip)
        0x3D, // dec a
        0xE0,
        0x82, // ldh [$FF82], a
        // === Read A button (P14=1, P15=0 -> write $10 to FF00) ===
        0x3E, 0x10, // ld a, $10
        0xE0, 0x00, // ldh [$FF00], a
        0xF0, 0x00, // ldh a, [$FF00] (settle)
        0xF0, 0x00, // ldh a, [$FF00] (read)
        0x2F,       // cpl (invert: 1=pressed)
        0xE6, 0x01, // and $01 (isolate A button, bit 0)
        0x4F,       // ld c, a (C = current A state)
        0xF0, 0x84, // ldh a, [$FF84] (prev state)
        0x57,       // ld d, a
        0x79,       // ld a, c
        0xE0, 0x84, // ldh [$FF84], a (update prev = current)
        0x7A,       // ld a, d
        0x2F,       // cpl
        0xA1,       // and c (newly pressed = curr & ~prev)
        0x28, 0x03, // jr z, +3 (skip CALL if not pressed)
    ]);

    // CALL save_routine - placeholder, address filled in after code is complete
    let call_save_patch_offset = code.len();
    code.extend_from_slice(&[
        0xCD, 0x00, 0x00, // CALL save_routine (patched later)
    ]);

    code.extend_from_slice(&[
        // === Select camera register bank (SRAM bank $10) ===
        0x3E,
        0x10, // ld a, $10
        0xEA,
        0x00,
        0x40, // ld [$4000], a
        // === Write camera registers ===
        // A001: Gain, N, VH
        0x3E,
        reg_a001, // ld a, reg_a001
        0xEA,
        0x01,
        0xA0, // ld [$A001], a
        // A002: Exposure low byte (from HRAM)
        0xF0,
        0x80, // ldh a, [$FF80]
        0xEA,
        0x02,
        0xA0, // ld [$A002], a
        // A003: Exposure high byte (from HRAM)
        0xF0,
        0x81, // ldh a, [$FF81]
        0xEA,
        0x03,
        0xA0, // ld [$A003], a
        // A004: Edge enhancement
        0x3E,
        reg_a004, // ld a, reg_a004
        0xEA,
        0x04,
        0xA0, // ld [$A004], a
        // A005: Voltage offset
        0x3E,
        config.voltage_offset, // ld a, offset
        0xEA,
        0x05,
        0xA0, // ld [$A005], a
        // === Compute dither matrix ROM address ===
        // HL = 0x0300 + contrast * 48
        0xF0,
        0x82, // ldh a, [$FF82]
        0x6F, // ld l, a
        0x26,
        0x00, // ld h, $00
        0x29, // add hl, hl (x2)
        0x29, // add hl, hl (x4)
        0x29, // add hl, hl (x8)
        0x29, // add hl, hl (x16)
        0x54, // ld d, h
        0x5D, // ld e, l (DE = contrast*16)
        0x29, // add hl, hl (x32)
        0x19, // add hl, de (x48)
        0x11,
        0x00,
        0x10, // ld de, $1000
        0x19, // add hl, de
        // === Copy 48-byte dither matrix to A006-A035 ===
        0x11,
        0x06,
        0xA0, // ld de, $A006
        0x06,
        0x30, // ld b, 48
        // dither_copy_loop:
        0x2A, // ld a, [hl+]
        0x12, // ld [de], a
        0x13, // inc de
        0x05, // dec b
        0x20,
        0xFA, // jr nz, dither_copy_loop (-6)
        // === Trigger capture ===
        0x3E,
        0x01, // ld a, $01
        0xEA,
        0x00,
        0xA0, // ld [$A000], a
        // === Wait for capture complete ===
        // wait_capture:
        0xFA,
        0x00,
        0xA0, // ld a, [$A000]
        0xE6,
        0x01, // and $01
        0x20,
        0xF9, // jr nz, wait_capture (-7)
        // === Switch to SRAM bank 0 (image data) ===
        0xAF, // xor a
        0xEA,
        0x00,
        0x40, // ld [$4000], a
        // === Copy SRAM image to VRAM for LCD display ===
        0x21,
        0x00,
        0xA1, // ld hl, $A100 (SRAM source)
        0x11,
        0x00,
        0x80, // ld de, $8000 (VRAM destination)
        0x01,
        0x00,
        0x0E, // ld bc, $0E00 (3584 bytes = 224 tiles x 16)
        // vram_copy_loop:
        0x2A, // ld a, [hl+]
        0x12, // ld [de], a
        0x13, // inc de
        0x0B, // dec bc
        0x78, // ld a, b
        0xB1, // or c
        0x20,
        0xF8, // jr nz, vram_copy_loop (-8)
        // === Loop back to CAPTURE_LOOP ===
        0xC3,
        (capture_loop_addr & 0xFF) as u8,
        (capture_loop_addr >> 8) as u8,
    ]);

    // === SAVE ROUTINE ===
    let save_routine_addr = 0x0150 + code.len() as u16;

    // Patch the CALL save_routine address
    code[call_save_patch_offset + 1] = (save_routine_addr & 0xFF) as u8;
    code[call_save_patch_offset + 2] = (save_routine_addr >> 8) as u8;

    code.extend_from_slice(&[
        // save_routine:
        // 1. Check remaining > 0
        0xF0, 0x85,       // ldh a, [$FF85] (remaining)
        0xB7,             // or a
        0xC8,             // ret z (no slots left)

        // 2. Read slot number, push for later
        0xF0, 0x86,       // ldh a, [$FF86] (next slot, 1-based)
        0xF5,             // push af

        // 3. Calculate dest bank: (slot-1)/2 + 1
        0x3D,             // dec a (slot-1)
        0xCB, 0x3F,       // srl a (divide by 2)
        0x3C,             // inc a (+ 1)
        0xE0, 0x87,       // ldh [$FF87], a (dest bank)

        // 4. Calculate dest addr high: ((slot-1)&1)*$10 + $A0
        0xF0, 0x86,       // ldh a, [$FF86]
        0x3D,             // dec a (slot-1)
        0xE6, 0x01,       // and $01
        0xCB, 0x37,       // swap a (0->0, 1->$10)
        0xC6, 0xA0,       // add $A0 (-> $A0 or $B0)
        0xE0, 0x88,       // ldh [$FF88], a (dest addr high)

        // 5. Select bank 0, copy 3584 bytes from $A100 -> WRAM $C000
        0xAF,             // xor a
        0xEA, 0x00, 0x40, // ld [$4000], a (bank 0)
        0x21, 0x00, 0xA1, // ld hl, $A100 (SRAM source)
        0x11, 0x00, 0xC0, // ld de, $C000 (WRAM dest)
        0x01, 0x00, 0x0E, // ld bc, $0E00 (3584 bytes)
        // copy_to_wram:
        0x2A,             // ld a, [hl+]
        0x12,             // ld [de], a
        0x13,             // inc de
        0x0B,             // dec bc
        0x78,             // ld a, b
        0xB1,             // or c
        0x20, 0xF8,       // jr nz, copy_to_wram (-8)

        // 6. Select dest bank, copy 3584 bytes from WRAM $C000 -> dest addr
        0xF0, 0x87,       // ldh a, [$FF87] (dest bank)
        0xEA, 0x00, 0x40, // ld [$4000], a
        0x21, 0x00, 0xC0, // ld hl, $C000 (WRAM source)
        0xF0, 0x88,       // ldh a, [$FF88] (dest addr high)
        0x57,             // ld d, a
        0x1E, 0x00,       // ld e, $00 (dest addr low = 0)
        0x01, 0x00, 0x0E, // ld bc, $0E00 (3584 bytes)
        // copy_to_sram:
        0x2A,             // ld a, [hl+]
        0x12,             // ld [de], a
        0x13,             // inc de
        0x0B,             // dec bc
        0x78,             // ld a, b
        0xB1,             // or c
        0x20, 0xF8,       // jr nz, copy_to_sram (-8)

        // 7. Select bank 0, mark state vector occupied
        0xAF,             // xor a
        0xEA, 0x00, 0x40, // ld [$4000], a (bank 0)
        0xF1,             // pop af (slot number)
        0x3D,             // dec a (slot-1 = index into state vector)
        0x5F,             // ld e, a
        0x16, 0x00,       // ld d, $00
        0x21, 0xB2, 0xB1, // ld hl, $B1B2
        0x19,             // add hl, de
        0x36, 0x00,       // ld [hl], $00 (mark occupied)

        // 8. Increment next slot (FF86)
        0xF0, 0x86,       // ldh a, [$FF86]
        0x3C,             // inc a
        0xE0, 0x86,       // ldh [$FF86], a

        // 9. Decrement remaining (FF85), convert to BCD, write tiles
        0xF0, 0x85,       // ldh a, [$FF85]
        0x3D,             // dec a
        0xE0, 0x85,       // ldh [$FF85], a
        // Convert A (0-30) to 2-digit BCD
        // tens digit: A / 10
        0x47,             // ld b, a (save value)
        0x0E, 0x00,       // ld c, 0 (tens counter)
        // div10_loop:
        0xFE, 0x0A,       // cp 10
        0x38, 0x05,       // jr c, +5 (done dividing)
        0xD6, 0x0A,       // sub 10
        0x0C,             // inc c
        0x18, 0xF7,       // jr, div10_loop (-9)
        // Now C = tens, A = ones
        // Write tens digit tile at $9A31
        0x47,             // ld b, a (save ones)
        0x79,             // ld a, c (tens)
        0xC6, 0xE8,       // add $E8 (tile base for '0')
        0xEA, 0x31, 0x9A, // ld [$9A31], a
        // Write ones digit tile at $9A32
        0x78,             // ld a, b (ones)
        0xC6, 0xE8,       // add $E8
        0xEA, 0x32, 0x9A, // ld [$9A32], a

        // 10. Return
        0xC9,             // ret
    ]);

    rom[0x150..0x150 + code.len()].copy_from_slice(&code);

    rom
}

fn print_usage() {
    println!("Usage: camera_test_rom [options]");
    println!();
    println!("Options:");
    println!("  --pattern <name>    Dither pattern (default: standard)");
    println!("                      Options: standard, 2x2, grid, maze, nest, fuzz,");
    println!("                               vertical, horizontal, diagonal");
    println!("  --contrast <0-15>   Contrast level (default: 9)");
    println!("  --high-light        Use high-light mode (default: low-light)");
    println!("  --exposure <hex>    Exposure time 0x0000-0xFFFF (default: 0x1000)");
    println!("  --gain <0-3>        Gain level 0=high, 3=low (default: 0)");
    println!("  --edge <0-7>        Edge enhancement (default: 0)");
    println!("  --offset <0-255>    Voltage offset (default: 128)");
    println!("  --invert            Invert output");
    println!("  --release           For release");
    println!("  --help              Show this help");
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut config = CameraConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            "--pattern" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --pattern requires a value");
                    std::process::exit(1);
                }
                config.pattern = DitherPattern::from_str(&args[i]).unwrap_or_else(|| {
                    eprintln!("Error: unknown pattern '{}'", args[i]);
                    std::process::exit(1);
                });
            }
            "--contrast" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --contrast requires a value");
                    std::process::exit(1);
                }
                config.contrast = args[i].parse::<u8>().unwrap_or(9).min(15);
            }
            "--high-light" => {
                config.high_light = true;
            }
            "--exposure" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --exposure requires a value");
                    std::process::exit(1);
                }
                let s = args[i].trim_start_matches("0x").trim_start_matches("0X");
                config.exposure = u16::from_str_radix(s, 16).unwrap_or(0x1000);
            }
            "--gain" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --gain requires a value");
                    std::process::exit(1);
                }
                config.gain = args[i].parse::<u8>().unwrap_or(0).min(3);
            }
            "--edge" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --edge requires a value");
                    std::process::exit(1);
                }
                config.edge_enhance = args[i].parse::<u8>().unwrap_or(0).min(7);
            }
            "--offset" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --offset requires a value");
                    std::process::exit(1);
                }
                config.voltage_offset = args[i].parse::<u8>().unwrap_or(128);
            }
            "--invert" => {
                config.invert = true;
            }
            "--release" => {
                config.release = true;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let rom = build_rom(&config);

    let output_path = if config.release {
        "../web/dist/pkg/film.gb"
    } else {
        "rom.gb"
    };
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(output_path)?;
    file.write_all(&rom)?;

    println!("Generated {} ({} bytes)", output_path, rom.len());
    println!();
    println!("{config}");
    println!("Controls:");
    println!("  Up:    Increase brightness (exposure +0x0400)");
    println!("  Down:  Decrease brightness (exposure -0x0400)");
    println!("  Right: Increase contrast (+1, max 15)");
    println!("  Left:  Decrease contrast (-1, min 0)");
    println!("  A:     Save photo to next slot (30 slots total)");
    println!();
    println!("Camera capture loop:");
    println!("  1. Reads D-PAD + A button input");
    println!("  2. Adjusts exposure/contrast, saves photo on A press");
    println!("  3. Configures camera registers at A001-A035");
    println!("  4. Triggers capture by writing 0x01 to A000");
    println!("  5. Polls A000 until bit 0 clears");
    println!("  6. Copies image from SRAM to VRAM for LCD display");
    println!("  7. Repeats continuously");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rom_code_size_within_bounds() {
        let config = CameraConfig::default();
        let rom = build_rom(&config);

        // Find last non-zero byte in code region (0x0150 to 0x05FF)
        let mut last_code_addr = 0x0150;
        for i in 0x0150..0x0600 {
            if rom[i] != 0 {
                last_code_addr = i;
            }
        }

        let code_size = last_code_addr - 0x0150 + 1;
        let space_before_font = 0x0600 - last_code_addr - 1;

        // Code must not overlap font data at 0x0600
        assert!(
            last_code_addr < 0x0600,
            "Code extends past 0x05FF into font data region! Last byte at 0x{:04X}",
            last_code_addr
        );

        // Verify font data at 0x0600 is present
        assert_eq!(
            rom[0x0600], 0xFF,
            "Font data at 0x0600 should start with 0xFF"
        );

        // Verify dither data is intact at 0x1000
        assert_ne!(rom[0x1000], 0, "Dither data at 0x1000 should be non-zero");

        println!(
            "Code size: {} bytes (0x0150-0x{:04X}), {} bytes spare before font data",
            code_size, last_code_addr, space_before_font
        );
    }

    #[test]
    fn test_rom_header_valid() {
        let config = CameraConfig::default();
        let rom = build_rom(&config);

        // Check entry point
        assert_eq!(rom[0x100], 0x00, "Expected NOP at entry point");
        assert_eq!(rom[0x101], 0xC3, "Expected JP at entry point");

        // Check Nintendo logo present
        assert_eq!(rom[0x104], 0xCE, "Nintendo logo should start at 0x104");

        // Check cartridge type (Pocket Camera)
        assert_eq!(
            rom[0x147], 0xFC,
            "Cartridge type should be 0xFC (Pocket Camera)"
        );

        // Verify header checksum
        let checksum: u8 = rom[0x134..=0x14C]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_sub(b).wrapping_sub(1));
        assert_eq!(rom[0x14D], checksum, "Header checksum mismatch");
    }
}
