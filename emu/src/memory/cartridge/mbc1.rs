//! MBC1 cartridge implementation.
//!
//! Supports up to 2MB ROM (128 banks) and 32KB RAM (4 banks).
//! Two modes: ROM banking (default) and RAM banking (mode bit set).

use super::{Cartridge, MbcType};

const ROM_BANK_SIZE: usize = 0x4000; // 16KB
const RAM_BANK_SIZE: usize = 0x2000; // 8KB

pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u16, // 5-bit bank number (upper 2 bits from 0x4000-0x5FFF in ROM mode)
    ram_bank: u8,
    ram_enabled: bool,
    mode: bool, // false = ROM banking mode, true = RAM banking mode
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Mbc1 {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            mode: false,
        }
    }
}

impl Cartridge for Mbc1 {
    fn read_rom(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF),
            0x4000..=0x7FFF => {
                let bank = self.rom_bank.max(1) as usize;
                let offset = bank * ROM_BANK_SIZE + (addr as usize - 0x4000);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write_rom(&mut self, addr: u16, value: u8) {
        match addr {
            // RAM enable: 0x0A in lower nibble enables
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            // ROM bank number (lower 5 bits); 0 → 1
            0x2000..=0x3FFF => {
                let bank = value & 0x1F;
                self.rom_bank =
                    (self.rom_bank & 0x60) | (if bank == 0 { 1 } else { bank }) as u16;
            }
            // Upper 2 bits of ROM bank or RAM bank select
            0x4000..=0x5FFF => {
                if self.mode {
                    self.ram_bank = value & 0x03;
                } else {
                    self.rom_bank = (self.rom_bank & 0x1F) | ((value as u16 & 0x03) << 5);
                }
            }
            // Banking mode select
            0x6000..=0x7FFF => self.mode = (value & 0x01) != 0,
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
        MbcType::Mbc1
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
