//! MBC3 cartridge implementation with Real-Time Clock support.
//!
//! Supports up to 2MB ROM (128 banks), 64KB RAM (8 banks), and an RTC
//! accessible via RAM bank registers 0x08-0x0C.

use super::{Cartridge, MbcType};
use crate::memory::rtc::Rtc;

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u16, // 7-bit bank number
    ram_bank: u8,  // 0x00-0x03 = RAM, 0x08-0x0C = RTC
    ram_enabled: bool,
    rtc: Rtc,
}

impl Mbc3 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Mbc3 {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            rtc: Rtc::new(),
        }
    }
}

impl Cartridge for Mbc3 {
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
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                let bank = value & 0x7F;
                self.rom_bank = if bank == 0 { 1 } else { bank as u16 };
            }
            // RAM bank or RTC register select
            0x4000..=0x5FFF => self.ram_bank = value,
            // RTC latch: write 0x00 then 0x01
            0x6000..=0x7FFF => self.rtc.write_latch(value),
            _ => {}
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if Rtc::is_rtc_register(self.ram_bank) {
            return self.rtc.read_register(self.ram_bank);
        }
        if !self.ram_enabled {
            return 0xFF;
        }
        let offset = self.ram_bank as usize * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        self.ram.get(offset).copied().unwrap_or(0xFF)
    }

    fn write_ram(&mut self, addr: u16, value: u8) {
        if Rtc::is_rtc_register(self.ram_bank) {
            self.rtc.write_register(self.ram_bank, value);
            return;
        }
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
        MbcType::Mbc3
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

    fn tick_rtc(&mut self) {
        self.rtc.tick();
    }
}
