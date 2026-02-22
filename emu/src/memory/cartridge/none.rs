//! No-MBC cartridge (ROM-only, 32KB max).

use super::{Cartridge, MbcType};

const ROM_BANK_SIZE: usize = 0x4000;

pub struct NoMbc {
    rom: Vec<u8>,
}

impl NoMbc {
    pub fn new(rom: Vec<u8>) -> Self {
        NoMbc { rom }
    }
}

impl Cartridge for NoMbc {
    fn read_rom(&self, addr: u16) -> u8 {
        self.rom.get(addr as usize).copied().unwrap_or(0xFF)
    }

    fn write_rom(&mut self, _addr: u16, _value: u8) {
        // No MBC registers
    }

    fn read_ram(&self, _addr: u16) -> u8 {
        0xFF // no external RAM
    }

    fn write_ram(&mut self, _addr: u16, _value: u8) {
        // no external RAM
    }

    fn ram_data(&self) -> &[u8] {
        &[]
    }

    fn load_ram(&mut self, _data: &[u8]) {}

    fn mbc_type(&self) -> MbcType {
        MbcType::None
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len() / ROM_BANK_SIZE
    }
}
