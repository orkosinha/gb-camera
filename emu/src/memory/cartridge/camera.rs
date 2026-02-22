//! Pocket Camera (Game Boy Camera) cartridge implementation.
//!
//! RAM banks 0x00-0x0F map to 128KB SRAM (photo storage).
//! RAM banks 0x10+ map to camera sensor registers (A000-A07F)
//! and captured tile data (A080-AFFF).
//!
//! SRAM is always accessible regardless of the RAM enable register,
//! matching real hardware behaviour.

use super::{Cartridge, MbcType};
use crate::log::{LogCategory, RateLimiter};
use crate::{log_info, log_info_limited};
use crate::memory::camera::Camera;

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

pub struct PocketCamera {
    rom: Vec<u8>,
    pub camera: Camera,
    rom_bank: u16, // 7-bit MBC3-compatible ROM bank
    ram_bank: u8,  // 0x00-0x0F = SRAM, 0x10+ = camera registers
}

impl PocketCamera {
    pub fn new(rom: Vec<u8>) -> Self {
        PocketCamera {
            rom,
            camera: Camera::new(),
            rom_bank: 1,
            ram_bank: 0,
        }
    }
}

impl Cartridge for PocketCamera {
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
            // RAM enable (ignored — SRAM is always accessible for PocketCamera)
            0x0000..=0x1FFF => {}
            // ROM bank select (7-bit, MBC3 compatible; bank 0 maps to bank 1)
            0x2000..=0x3FFF => {
                let bank = value & 0x7F;
                self.rom_bank = if bank == 0 { 1 } else { bank as u16 };
            }
            // RAM bank / camera register bank select (5-bit)
            0x4000..=0x5FFF => {
                let new_bank = value & 0x1F;
                log_info!(
                    LogCategory::Camera,
                    "RAM bank: {} -> {} (mode={})",
                    self.ram_bank,
                    new_bank,
                    if new_bank >= 0x10 { "CAMERA_REGS" } else { "SRAM" }
                );
                self.ram_bank = new_bank;
            }
            _ => {}
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        // Bank >= 0x10: camera registers / tile data overlay
        if self.ram_bank >= 0x10 {
            let reg_addr = (addr - 0xA000) as usize;
            if reg_addr < 0x80 {
                let value = self.camera.regs[reg_addr];
                if reg_addr == 0 {
                    static A000_READ_LIMITER: RateLimiter = RateLimiter::new(50);
                    log_info_limited!(
                        LogCategory::Camera,
                        &A000_READ_LIMITER,
                        "Read A000 (capture status): 0x{:02X} (busy={})",
                        value,
                        (value & 0x01) != 0
                    );
                }
                return value;
            }
            // A080-AFFF: captured tile data mapped from SRAM offset 0x0100
            let tile_offset = reg_addr - 0x80;
            if tile_offset < 0x0E00 {
                let sram_addr = 0x0100 + tile_offset;
                let value = self.camera.ram.get(sram_addr).copied().unwrap_or(0x00);
                static TILE_READ_LIMITER: RateLimiter = RateLimiter::new(20);
                log_info_limited!(
                    LogCategory::Camera,
                    &TILE_READ_LIMITER,
                    "Read camera tile data A{:03X} -> SRAM[{:04X}] = {:02X}",
                    reg_addr,
                    sram_addr,
                    value
                );
                return value;
            }
            return 0x00;
        }

        // Banks 0x00-0x0F: SRAM access (always enabled)
        let offset = self.ram_bank as usize * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        let value = self.camera.ram.get(offset).copied().unwrap_or(0x00);

        if (0xA100..0xAF00).contains(&addr) {
            static SRAM_READ_LIMITER: RateLimiter = RateLimiter::new(50);
            log_info_limited!(
                LogCategory::Camera,
                &SRAM_READ_LIMITER,
                "SRAM read: {:04X} bank={} offset={:04X} -> {:02X}",
                addr,
                self.ram_bank,
                offset,
                value
            );
        }

        value
    }

    fn write_ram(&mut self, addr: u16, value: u8) {
        // Bank >= 0x10: camera register writes
        if self.ram_bank >= 0x10 {
            let reg_addr = (addr - 0xA000) as usize;
            if reg_addr < 0x80 {
                if reg_addr == 0 {
                    log_info!(
                        LogCategory::Camera,
                        "Write A000: 0x{:02X} (capture={}, N={}, VH={})",
                        value,
                        (value & 0x01) != 0,
                        (value >> 1) & 0x01,
                        (value >> 2) & 0x03
                    );
                } else if reg_addr <= 0x35 {
                    static REG_LIMITER: RateLimiter = RateLimiter::new(100);
                    log_info_limited!(
                        LogCategory::Camera,
                        &REG_LIMITER,
                        "Write A0{:02X}: 0x{:02X}",
                        reg_addr,
                        value
                    );
                }

                self.camera.regs[reg_addr] = value;
                // Register 0 bit 0: 1 = start capture
                if reg_addr == 0 && (value & 0x01) != 0 {
                    let invert = (value & 0x02) != 0;
                    log_info!(
                        LogCategory::Camera,
                        "Capture triggered! image_ready={}, invert={}, Processing...",
                        self.camera.image_ready,
                        invert
                    );
                    self.camera.process_capture(invert);
                    self.camera.capture_dirty = true;
                    self.camera.regs[0] &= !0x01;
                    log_info!(
                        LogCategory::Camera,
                        "Capture complete, A000 now=0x{:02X}",
                        self.camera.regs[0]
                    );
                }
            }
            return;
        }

        // Banks 0x00-0x0F: SRAM write (always enabled)
        let offset = self.ram_bank as usize * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        if offset < self.camera.ram.len() {
            self.camera.ram[offset] = value;
        }
    }

    fn ram_data(&self) -> &[u8] {
        &self.camera.ram
    }

    fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.camera.ram.len());
        self.camera.ram[..len].copy_from_slice(&data[..len]);
    }

    fn mbc_type(&self) -> MbcType {
        MbcType::PocketCamera
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

    fn as_camera(&self) -> Option<&Camera> {
        Some(&self.camera)
    }

    fn as_camera_mut(&mut self) -> Option<&mut Camera> {
        Some(&mut self.camera)
    }
}
