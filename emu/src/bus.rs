use crate::joypad::Joypad;
use crate::memory::Memory;
use crate::timer::Timer;

/// MemoryBus routes memory accesses to the appropriate component.
/// This ensures Timer and Joypad registers are properly integrated.
pub struct MemoryBus<'a> {
    pub memory: &'a mut Memory,
    pub timer: &'a mut Timer,
    pub joypad: &'a mut Joypad,
}

impl<'a> MemoryBus<'a> {
    pub fn new(memory: &'a mut Memory, timer: &'a mut Timer, joypad: &'a mut Joypad) -> Self {
        MemoryBus {
            memory,
            timer,
            joypad,
        }
    }

    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Joypad register
            0xFF00 => self.joypad.read(),
            // Timer registers
            0xFF04..=0xFF07 => self.timer.read(addr),
            // All other addresses go to memory
            _ => self.memory.read(addr),
        }
    }

    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            // Joypad register
            0xFF00 => self.joypad.write(value),
            // Timer registers
            0xFF04..=0xFF07 => self.timer.write(addr, value),
            // All other addresses go to memory
            _ => self.memory.write(addr, value),
        }
    }

    // Delegate methods needed by other components
    #[inline]
    pub fn get_ie(&self) -> u8 {
        self.memory.get_ie()
    }

    #[inline]
    pub fn read_io_direct(&self, offset: u8) -> u8 {
        self.memory.read_io_direct(offset)
    }

    #[allow(dead_code)]
    pub fn write_io_direct(&mut self, offset: u8, value: u8) {
        self.memory.write_io_direct(offset, value)
    }

    #[allow(dead_code)]
    pub fn get_oam(&self) -> &[u8] {
        self.memory.get_oam()
    }
}
