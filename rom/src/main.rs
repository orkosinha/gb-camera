//! Minimal Game Boy Camera test ROM generator.
//!
//! Generates a ROM that continuously captures camera images to SRAM.
//! Uses authentic dithering patterns and contrast settings from gb-photo.
//!
//! The captured image data is stored at SRAM offset 0x0100 (address 0xA100).
//! Format: 128x112 pixels as 2bpp tiles (16x14 tiles, 3584 bytes).

use std::env;
use std::fs::File;
use std::io::Write;

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
        let offset = 0x0300 + (level as usize) * 48;
        rom[offset..offset + 48].copy_from_slice(&matrix);
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
    let code: Vec<u8> = vec![
        // === INIT (0x0150) ===
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
        // === CAPTURE_LOOP (0x0161) ===
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
        0x03, // ld de, $0300
        0x19, // add hl, de
        // === Copy 48-byte dither matrix to A006-A035 ===
        0x11,
        0x06,
        0xA0, // ld de, $A006
        0x06,
        0x30, // ld b, 48
        // copy_loop:
        0x2A, // ld a, [hl+]
        0x12, // ld [de], a
        0x13, // inc de
        0x05, // dec b
        0x20,
        0xFA, // jr nz, copy_loop (-6)
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
        // === Small delay before next capture ===
        0x01,
        0x00,
        0x10, // ld bc, $1000
        // delay_loop:
        0x0B, // dec bc
        0x78, // ld a, b
        0xB1, // or c
        0x20,
        0xFB, // jr nz, delay_loop (-5)
        // === Loop back to CAPTURE_LOOP (0x0161) ===
        0xC3,
        0x61,
        0x01, // jp $0161
    ];

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
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let rom = build_rom(&config);

    let output_path = "rom.gb";
    let mut file = File::create(output_path)?;
    file.write_all(&rom)?;

    println!("Generated {} ({} bytes)", output_path, rom.len());
    println!();
    println!("{config}");
    println!("D-PAD controls (real-time adjustment):");
    println!("  Up:    Increase brightness (exposure +0x0400)");
    println!("  Down:  Decrease brightness (exposure -0x0400)");
    println!("  Right: Increase contrast (+1, max 15)");
    println!("  Left:  Decrease contrast (-1, min 0)");
    println!();
    println!("Camera capture loop:");
    println!("  1. Reads D-PAD input and adjusts exposure/contrast");
    println!("  2. Configures camera registers at A001-A035");
    println!("  3. Triggers capture by writing 0x01 to A000");
    println!("  4. Polls A000 until bit 0 clears");
    println!("  5. Captured image at SRAM 0xA100 (3584 bytes, 2bpp tiles)");
    println!("  6. Repeats continuously");

    Ok(())
}
