//! Memory bus that routes reads and writes to the correct component.
//!
//! The Game Boy's memory map is shared between the CPU, timer, joypad, and
//! general-purpose RAM/ROM. [`MemoryBus`] intercepts accesses to hardware
//! register addresses and delegates to the owning component.

use crate::joypad::Joypad;
use crate::memory::Memory;
use crate::timer::Timer;

/// MemoryBus routes memory accesses to the appropriate component.
/// This ensures Timer and Joypad registers are properly integrated.
pub struct MemoryBus<'a> {
    memory: &'a mut Memory,
    timer: &'a mut Timer,
    joypad: &'a mut Joypad,
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

    #[inline]
    pub fn memory_mut(&mut self) -> &mut Memory {
        self.memory
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

}
