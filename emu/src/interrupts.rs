//! Interrupt controller for the Game Boy's five interrupt sources.
//!
//! Interrupts are requested by writing to the IF register (0xFF0F) and
//! enabled via the IE register (0xFFFF). Priority order (highest first):
//! VBlank, LCD STAT, Timer, Serial, Joypad.

use crate::memory::{Memory, io};

/// Game Boy interrupt types, ordered by hardware priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Interrupt {
    VBlank = 0,
    LcdStat = 1,
    Timer = 2,
    Serial = 3,
    Joypad = 4,
}

/// Interrupt controller that operates directly on memory's IF register (0xFF0F).
/// This ensures the CPU always sees the correct interrupt state.
#[derive(Default)]
pub struct InterruptController;

impl InterruptController {
    pub fn new() -> Self {
        InterruptController
    }

    /// Set the interrupt flag bit for the given interrupt type.
    #[inline]
    pub fn request(&self, interrupt: Interrupt, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(io::IF);
        memory.write_io_direct(io::IF, if_reg | (1 << interrupt as u8));
    }

    /// Clear the interrupt flag bit for the given interrupt type.
    #[inline]
    pub fn clear(&self, interrupt: Interrupt, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(io::IF);
        memory.write_io_direct(io::IF, if_reg & !(1 << interrupt as u8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_requests() {
        let ic = InterruptController::new();
        let mut mem = Memory::new();
        mem.write_io_direct(io::IF, 0x00);

        ic.request(Interrupt::VBlank, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF) & 0x01, 0x01);

        ic.request(Interrupt::LcdStat, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF) & 0x02, 0x02);

        ic.request(Interrupt::Timer, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF) & 0x04, 0x04);

        ic.request(Interrupt::Serial, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF) & 0x08, 0x08);

        ic.request(Interrupt::Joypad, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF) & 0x10, 0x10);
    }

    #[test]
    fn test_interrupt_clear() {
        let ic = InterruptController::new();
        let mut mem = Memory::new();
        mem.write_io_direct(io::IF, 0x1F); // All interrupts set

        ic.clear(Interrupt::VBlank, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF), 0x1E);

        ic.clear(Interrupt::Timer, &mut mem);
        assert_eq!(mem.read_io_direct(io::IF), 0x1A);
    }
}
