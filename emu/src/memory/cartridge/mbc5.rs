//! MBC5 cartridge implementation.
//!
//! Supports up to 8MB ROM (512 banks, 9-bit bank number) and 128KB RAM
//! (16 banks, 4-bit bank number).

use super::{Cartridge, MbcType};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

pub struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u16, // 9-bit bank number (low 8 + high 1 bit)
    ram_bank: u8,  // 4-bit bank number
    ram_enabled: bool,
}

impl Mbc5 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Mbc5 {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
        }
    }
}

impl Cartridge for Mbc5 {
    fn read_rom(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF),
            0x4000..=0x7FFF => {
                let bank = self.rom_bank as usize;
                let offset = bank * ROM_BANK_SIZE + (addr as usize - 0x4000);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write_rom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            // Low 8 bits of ROM bank number (0x2000-0x2FFF)
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | (value as u16);
            }
            // High bit of ROM bank number (0x3000-0x3FFF)
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0xFF) | ((value as u16 & 1) << 8);
            }
            // RAM bank select (4-bit)
            0x4000..=0x5FFF => self.ram_bank = value & 0x0F,
            _ => {}
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        let offset = self.ram_bank as usize * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        self.ram.get(offset).copied().unwrap_or(0xFF)
    }

    fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }
        let offset = self.ram_bank as usize * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        if offset < self.ram.len() {
            self.ram[offset] = value;
        }
    }

    fn ram_data(&self) -> &[u8] {
        &self.ram
    }

    fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }

    fn mbc_type(&self) -> MbcType {
        MbcType::Mbc5
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len() / ROM_BANK_SIZE
    }

    fn current_rom_bank(&self) -> u16 {
        self.rom_bank
    }

    fn current_ram_bank(&self) -> u8 {
        self.ram_bank
    }

    fn is_ram_enabled(&self) -> bool {
        self.ram_enabled
    }
}
