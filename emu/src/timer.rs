//! Game Boy timer emulation (DIV, TIMA, TMA, TAC registers).
//!
//! The timer increments TIMA at a configurable frequency (4096–262144 Hz)
//! controlled by TAC. When TIMA overflows, it reloads from TMA and
//! requests a Timer interrupt. DIV increments at a fixed 16384 Hz rate.

use crate::interrupts::{Interrupt, InterruptController};
use crate::memory::Memory;

pub struct Timer {
    div_counter: u16,    // Internal 16-bit counter, DIV is upper 8 bits
    tima: u8,            // Timer counter (0xFF05)
    tma: u8,             // Timer modulo (0xFF06)
    tac: u8,             // Timer control (0xFF07)
    overflow_cycles: u8, // Cycles until TIMA reload after overflow
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div_counter: 0xABCC, // Initial value after boot
            tima: 0,
            tma: 0,
            tac: 0xF8,
            overflow_cycles: 0,
        }
    }

    pub fn tick(&mut self, cycles: u32, memory: &mut Memory, interrupts: &InterruptController) {
        for _ in 0..cycles {
            self.tick_once(memory, interrupts);
        }
    }

    #[inline]
    fn tick_once(&mut self, memory: &mut Memory, interrupts: &InterruptController) {
        // Handle delayed TIMA reload
        if self.overflow_cycles > 0 {
            self.overflow_cycles -= 1;
            if self.overflow_cycles == 0 {
                self.tima = self.tma;
                interrupts.request(Interrupt::Timer, memory);
            }
        }

        let old_div = self.div_counter;
        self.div_counter = self.div_counter.wrapping_add(1);

        // Check if timer is enabled
        if self.tac & 0x04 == 0 {
            return;
        }

        // Get the bit position to check based on clock select
        let bit = match self.tac & 0x03 {
            0 => 9, // 4096 Hz (DIV bit 9)
            1 => 3, // 262144 Hz (DIV bit 3)
            2 => 5, // 65536 Hz (DIV bit 5)
            3 => 7, // 16384 Hz (DIV bit 7)
            _ => unreachable!(),
        };

        // Falling edge detection
        let old_bit = (old_div >> bit) & 1;
        let new_bit = (self.div_counter >> bit) & 1;

        if old_bit == 1 && new_bit == 0 {
            self.tima = self.tima.wrapping_add(1);
            if self.tima == 0 {
                // Overflow - delay reload by 4 cycles
                self.overflow_cycles = 4;
            }
        }
    }

    /// Read timer registers (0xFF04-0xFF07).
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div_counter >> 8) as u8, // DIV
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper bits always 1
            _ => 0xFF,
        }
    }

    /// Write timer registers (0xFF04-0xFF07).
    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => self.div_counter = 0, // Writing any value resets DIV
            0xFF05 => {
                // Writing to TIMA during overflow delay cancels the interrupt
                if self.overflow_cycles > 0 {
                    self.overflow_cycles = 0;
                }
                self.tima = value;
            }
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value,
            _ => {}
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_increment() {
        let mut timer = Timer::new();
        let mut mem = Memory::new();
        let ic = InterruptController::new();

        let initial_div = timer.read(0xFF04);
        timer.tick(256, &mut mem, &ic);
        let new_div = timer.read(0xFF04);

        assert_eq!(new_div, initial_div.wrapping_add(1));
    }

    #[test]
    fn test_div_reset() {
        let mut timer = Timer::new();
        timer.div_counter = 0x1234;

        timer.write(0xFF04, 0xFF);
        assert_eq!(timer.div_counter, 0);
    }

    #[test]
    fn test_timer_disabled() {
        let mut timer = Timer::new();
        let mut mem = Memory::new();
        let ic = InterruptController::new();

        timer.tac = 0x00; // Timer disabled
        timer.tima = 0;

        timer.tick(1000, &mut mem, &ic);
        assert_eq!(timer.tima, 0);
    }
}
