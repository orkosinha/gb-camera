//! Cartridge abstraction: `Cartridge` trait + concrete MBC implementations.
//!
//! Each cartridge type owns its ROM, RAM, and banking state. `Memory` holds a
//! `Box<dyn Cartridge>` and delegates all 0x0000-0x7FFF and 0xA000-0xBFFF
//! accesses through it.

mod camera;
mod mbc1;
mod mbc3;
mod mbc5;
mod mbc7;
mod none;

pub use camera::PocketCamera;
pub use mbc1::Mbc1;
pub use mbc3::Mbc3;
pub use mbc5::Mbc5;
pub use mbc7::Mbc7;
pub use none::NoMbc;

use super::camera::Camera;

/// Cartridge/MBC type identifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MbcType {
    None,         // No MBC (32KB ROM only)
    Mbc1,         // MBC1
    Mbc3,         // MBC3 (with RTC support)
    Mbc5,         // MBC5
    Mbc7,         // MBC7 (accelerometer + EEPROM; Kirby's Tilt 'n' Tumble)
    PocketCamera, // Game Boy Camera (0xFC)
}

/// Abstraction over cartridge hardware (ROM chips + MBC + RAM).
///
/// Implementations own all banking state; `Memory` is a thin router.
pub trait Cartridge {
    /// Read from ROM address space (0x0000-0x7FFF).
    fn read_rom(&self, addr: u16) -> u8;
    /// Write to MBC registers (0x0000-0x7FFF). Does not write ROM.
    fn write_rom(&mut self, addr: u16, value: u8);
    /// Read from external RAM (0xA000-0xBFFF).
    fn read_ram(&self, addr: u16) -> u8;
    /// Write to external RAM (0xA000-0xBFFF).
    fn write_ram(&mut self, addr: u16, value: u8);
    /// Borrow the full cartridge RAM slice (for save data export).
    fn ram_data(&self) -> &[u8];
    /// Load save data into cartridge RAM (truncated if too long).
    fn load_ram(&mut self, data: &[u8]);
    /// MBC type identifier.
    fn mbc_type(&self) -> MbcType;
    /// Total number of 16KB ROM banks.
    fn rom_bank_count(&self) -> usize;
    /// Currently selected ROM bank (for debug).
    fn current_rom_bank(&self) -> u16 {
        1
    }
    /// Currently selected RAM bank (for debug).
    fn current_ram_bank(&self) -> u8 {
        0
    }
    /// Whether external RAM is enabled (for debug).
    fn is_ram_enabled(&self) -> bool {
        false
    }
    /// Advance the RTC by wall-clock time (no-op for non-MBC3 cartridges).
    fn tick_rtc(&mut self) {}
    /// Return the inner `Camera` if this is a Pocket Camera cartridge.
    fn as_camera(&self) -> Option<&Camera> {
        None
    }
    /// Return the inner `Camera` mutably if this is a Pocket Camera cartridge.
    fn as_camera_mut(&mut self) -> Option<&mut Camera> {
        None
    }
    /// Return inner `Mbc7` mutably (for accelerometer input). Default: None.
    fn as_mbc7_mut(&mut self) -> Option<&mut Mbc7> {
        None
    }
}

/// Determine RAM size from cartridge header byte 0x0149.
pub fn ram_size_from_header(byte: u8) -> usize {
    match byte {
        0x00 => 8 * 1024,   // Default 8KB (some games report 0 but have RAM)
        0x01 => 2 * 1024,   // 2KB (unofficial)
        0x02 => 8 * 1024,   // 8KB
        0x03 => 32 * 1024,  // 32KB (4 banks)
        0x04 => 128 * 1024, // 128KB (16 banks)
        0x05 => 64 * 1024,  // 64KB (8 banks)
        _ => 128 * 1024,
    }
}

/// Create the appropriate cartridge implementation for a given ROM.
pub fn make_cartridge(rom: Vec<u8>, cart_type: u8, ram_size: usize) -> Box<dyn Cartridge> {
    match cart_type {
        0x00 => Box::new(NoMbc::new(rom)),
        0x01..=0x03 => Box::new(Mbc1::new(rom, ram_size)),
        0x0F..=0x13 => Box::new(Mbc3::new(rom, ram_size)),
        0x19..=0x1E => Box::new(Mbc5::new(rom, ram_size)),
        0x22        => Box::new(Mbc7::new(rom)),
        0xFC        => Box::new(PocketCamera::new(rom)),
        _ => Box::new(Mbc5::new(rom, ram_size)), // safe default for unknown types
    }
}
