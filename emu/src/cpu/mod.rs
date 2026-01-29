//! Sharp LR35902 CPU emulation.
//!
//! Implements the full instruction set including CB-prefixed opcodes,
//! interrupt handling, and the HALT state. Each `step` call executes
//! one instruction and returns the number of T-cycles consumed.

mod alu;
mod opcodes;

use std::fmt;

use crate::bus::MemoryBus;
use crate::interrupts::{Interrupt, InterruptController};
use crate::log::LogCategory;
use crate::log_info;
use crate::memory::io;

/// Debug state for CPU inspection.
#[allow(dead_code)]
pub struct CpuDebugState {
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub f: u8,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ime: bool,
    pub halted: bool,
}

impl fmt::Display for CpuDebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PC={:04X} SP={:04X} A={:02X} F={:02X} BC={:04X} DE={:04X} HL={:04X} IME={} halt={}",
            self.pc, self.sp, self.a, self.f, self.bc, self.de, self.hl, self.ime, self.halted
        )
    }
}

pub struct Cpu {
    // 8-bit registers
    a: u8,
    f: u8, // Flags: Z N H C 0 0 0 0
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,

    // 16-bit registers
    sp: u16,
    pc: u16,

    // State
    halted: bool,
    ime: bool,         // Interrupt Master Enable
    ime_pending: bool, // EI enables IME after next instruction

    // Debug
    instruction_count: u64,
}

// Flag bit positions
const FLAG_Z: u8 = 7;
const FLAG_N: u8 = 6;
const FLAG_H: u8 = 5;
const FLAG_C: u8 = 4;

impl Cpu {
    pub fn new() -> Self {
        // Initial values after boot ROM
        Cpu {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100, // Entry point after boot ROM
            halted: false,
            ime: true,
            ime_pending: false,
            instruction_count: 0,
        }
    }

    pub fn step(&mut self, bus: &mut MemoryBus, interrupts: &mut InterruptController) -> u32 {
        // Handle pending IME enable
        if self.ime_pending {
            self.ime = true;
            self.ime_pending = false;
        }

        // Handle interrupts
        if self.ime
            && let Some(cycles) = self.handle_interrupt(bus, interrupts)
        {
            return cycles;
        }

        // Wake from halt if any interrupt is pending
        if self.halted {
            let ie = bus.get_ie();
            let if_reg = bus.read_io_direct(io::IF);
            if ie & if_reg & 0x1F != 0 {
                self.halted = false;
            } else {
                return 4; // HALT consumes 4 cycles while waiting
            }
        }

        // Trace first 20 instructions
        let pc_before = self.pc;
        let opcode = self.fetch(bus);

        if self.instruction_count < 20 {
            log_info!(
                LogCategory::Cpu,
                "#{:04x}: PC=0x{:04X} OP=0x{:02X} A={:02X} BC={:04X} DE={:04X} HL={:04X} SP={:04X}",
                self.instruction_count,
                pc_before,
                opcode,
                self.a,
                self.bc(),
                self.de(),
                self.hl(),
                self.sp
            );
        }

        self.instruction_count += 1;
        self.execute(opcode, bus)
    }

    #[inline]
    fn fetch(&mut self, bus: &MemoryBus) -> u8 {
        let opcode = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        opcode
    }

    #[inline]
    fn fetch_word(&mut self, bus: &MemoryBus) -> u16 {
        let low = self.fetch(bus) as u16;
        let high = self.fetch(bus) as u16;
        (high << 8) | low
    }

    fn handle_interrupt(
        &mut self,
        bus: &mut MemoryBus,
        interrupts: &mut InterruptController,
    ) -> Option<u32> {
        let ie = bus.get_ie();
        let if_reg = bus.read_io_direct(io::IF);
        let pending = ie & if_reg & 0x1F;

        if pending == 0 {
            return None;
        }

        self.halted = false;
        self.ime = false;

        // Priority: VBlank > LCD STAT > Timer > Serial > Joypad
        let (interrupt, vector) = if pending & 0x01 != 0 {
            (Interrupt::VBlank, 0x0040)
        } else if pending & 0x02 != 0 {
            (Interrupt::LcdStat, 0x0048)
        } else if pending & 0x04 != 0 {
            (Interrupt::Timer, 0x0050)
        } else if pending & 0x08 != 0 {
            (Interrupt::Serial, 0x0058)
        } else {
            (Interrupt::Joypad, 0x0060)
        };

        // Clear interrupt flag
        interrupts.clear(interrupt, bus.memory_mut());

        // Push PC and jump to handler
        self.push_word(bus, self.pc);
        self.pc = vector;

        Some(20) // Interrupt handling takes 20 cycles
    }

    #[inline]
    fn get_reg(&self, idx: u8, bus: &MemoryBus) -> u8 {
        match idx {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => bus.read(self.hl()),
            7 => self.a,
            _ => 0,
        }
    }

    #[inline]
    fn set_reg(&mut self, idx: u8, value: u8, bus: &mut MemoryBus) {
        match idx {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => bus.write(self.hl(), value),
            7 => self.a = value,
            _ => {}
        }
    }

    // Register pair accessors
    #[inline]
    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }
    #[inline]
    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }
    #[inline]
    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }
    #[inline]
    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    #[inline]
    fn set_af(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f = (v & 0xF0) as u8;
    }
    #[inline]
    fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = v as u8;
    }
    #[inline]
    fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = v as u8;
    }
    #[inline]
    fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = v as u8;
    }

    // Flag accessors
    #[inline]
    fn flag(&self, bit: u8) -> bool {
        (self.f >> bit) & 1 == 1
    }
    #[inline]
    fn set_flag(&mut self, bit: u8, value: bool) {
        if value {
            self.f |= 1 << bit;
        } else {
            self.f &= !(1 << bit);
        }
    }

    // Stack operations
    #[inline]
    fn push_word(&mut self, bus: &mut MemoryBus, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, (value >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, value as u8);
    }

    #[inline]
    fn pop_word(&mut self, bus: &MemoryBus) -> u16 {
        let low = bus.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let high = bus.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        (high << 8) | low
    }

    /// Get current CPU state for debugging.
    #[allow(dead_code)]
    pub fn get_debug_state(&self) -> CpuDebugState {
        CpuDebugState {
            pc: self.pc,
            sp: self.sp,
            a: self.a,
            f: self.f,
            bc: self.bc(),
            de: self.de(),
            hl: self.hl(),
            ime: self.ime,
            halted: self.halted,
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interrupts::InterruptController;
    use crate::joypad::Joypad;
    use crate::memory::Memory;
    use crate::timer::Timer;

    struct TestContext {
        cpu: Cpu,
        memory: Memory,
        timer: Timer,
        joypad: Joypad,
        ic: InterruptController,
    }

    impl TestContext {
        fn step(&mut self) -> u32 {
            let mut bus = MemoryBus::new(&mut self.memory, &mut self.timer, &mut self.joypad);
            self.cpu.step(&mut bus, &mut self.ic)
        }
    }

    fn setup_with_rom(rom_data: &[u8]) -> TestContext {
        let mut mem = Memory::new();
        // Create a ROM with header and our test data starting at 0x100
        let mut rom = vec![0u8; 0x8000];
        // Copy test data to ROM entry point
        for (i, &byte) in rom_data.iter().enumerate() {
            if 0x100 + i < rom.len() {
                rom[0x100 + i] = byte;
            }
        }
        mem.load_rom(&rom).unwrap();
        TestContext {
            cpu: Cpu::new(),
            memory: mem,
            timer: Timer::new(),
            joypad: Joypad::new(),
            ic: InterruptController::new(),
        }
    }

    #[test]
    fn test_initial_state() {
        let cpu = Cpu::new();
        assert_eq!(cpu.pc, 0x0100);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.a, 0x01);
        assert!(!cpu.halted);
        assert!(cpu.ime);
    }

    #[test]
    fn test_nop() {
        let mut ctx = setup_with_rom(&[0x00]); // NOP
        let cycles = ctx.step();
        assert_eq!(cycles, 4);
        assert_eq!(ctx.cpu.pc, 0x0101);
    }

    #[test]
    fn test_ld_immediate_8bit() {
        let mut ctx = setup_with_rom(&[
            0x3E, 0x42, // LD A, 0x42
            0x06, 0x55, // LD B, 0x55
        ]);
        ctx.step();
        assert_eq!(ctx.cpu.a, 0x42);

        ctx.step();
        assert_eq!(ctx.cpu.b, 0x55);
    }

    #[test]
    fn test_ld_immediate_16bit() {
        let mut ctx = setup_with_rom(&[
            0x21, 0x34, 0x12, // LD HL, 0x1234
        ]);
        ctx.step();
        assert_eq!(ctx.cpu.hl(), 0x1234);
    }

    #[test]
    fn test_inc_dec_8bit() {
        let mut ctx = setup_with_rom(&[
            0x3C, // INC A
            0x3D, // DEC A
        ]);
        ctx.cpu.a = 0x0F;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x10);
        assert!(ctx.cpu.flag(FLAG_H)); // Half carry from 0x0F to 0x10

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x0F);
    }

    #[test]
    fn test_inc_zero_flag() {
        let mut ctx = setup_with_rom(&[0x3C]); // INC A
        ctx.cpu.a = 0xFF;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x00);
        assert!(ctx.cpu.flag(FLAG_Z));
    }

    #[test]
    fn test_add() {
        let mut ctx = setup_with_rom(&[0x80]); // ADD A, B
        ctx.cpu.a = 0x3A;
        ctx.cpu.b = 0xC6;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x00);
        assert!(ctx.cpu.flag(FLAG_Z));
        assert!(ctx.cpu.flag(FLAG_H));
        assert!(ctx.cpu.flag(FLAG_C));
    }

    #[test]
    fn test_sub() {
        let mut ctx = setup_with_rom(&[0x90]); // SUB B
        ctx.cpu.a = 0x3E;
        ctx.cpu.b = 0x3E;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x00);
        assert!(ctx.cpu.flag(FLAG_Z));
        assert!(ctx.cpu.flag(FLAG_N));
    }

    #[test]
    fn test_and() {
        let mut ctx = setup_with_rom(&[0xA0]); // AND B
        ctx.cpu.a = 0x5A;
        ctx.cpu.b = 0x3F;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x1A);
        assert!(ctx.cpu.flag(FLAG_H));
        assert!(!ctx.cpu.flag(FLAG_C));
    }

    #[test]
    fn test_xor() {
        let mut ctx = setup_with_rom(&[0xAF]); // XOR A
        ctx.cpu.a = 0xFF;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x00);
        assert!(ctx.cpu.flag(FLAG_Z));
    }

    #[test]
    fn test_jp() {
        let mut ctx = setup_with_rom(&[
            0xC3, 0x00, 0x80, // JP 0x8000
        ]);
        ctx.step();
        assert_eq!(ctx.cpu.pc, 0x8000);
    }

    #[test]
    fn test_jr() {
        let mut ctx = setup_with_rom(&[
            0x18, 0x05, // JR +5
        ]);
        ctx.step();
        assert_eq!(ctx.cpu.pc, 0x0107); // 0x0102 + 5
    }

    #[test]
    fn test_call_ret() {
        // Put RET at 0x200 (offset 0x100 in ROM)
        let mut rom_data = vec![0u8; 0x200];
        rom_data[0] = 0xCD; // CALL 0x200
        rom_data[1] = 0x00;
        rom_data[2] = 0x02;
        rom_data[0x100] = 0xC9; // RET at 0x200

        let mut ctx = setup_with_rom(&rom_data);
        ctx.cpu.sp = 0xFFFE;

        ctx.step();
        assert_eq!(ctx.cpu.pc, 0x0200);
        assert_eq!(ctx.cpu.sp, 0xFFFC);

        ctx.step();
        assert_eq!(ctx.cpu.pc, 0x0103);
        assert_eq!(ctx.cpu.sp, 0xFFFE);
    }

    #[test]
    fn test_push_pop() {
        let mut ctx = setup_with_rom(&[
            0xC5, // PUSH BC
            0xC1, // POP BC
        ]);
        ctx.cpu.sp = 0xFFFE;
        ctx.cpu.set_bc(0x1234);

        ctx.step();
        assert_eq!(ctx.cpu.sp, 0xFFFC);

        ctx.cpu.set_bc(0x0000);
        ctx.step();
        assert_eq!(ctx.cpu.bc(), 0x1234);
        assert_eq!(ctx.cpu.sp, 0xFFFE);
    }

    #[test]
    fn test_rlca() {
        let mut ctx = setup_with_rom(&[0x07]); // RLCA
        ctx.cpu.a = 0x85; // 10000101

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x0B); // 00001011
        assert!(ctx.cpu.flag(FLAG_C));
    }

    #[test]
    fn test_rrca() {
        let mut ctx = setup_with_rom(&[0x0F]); // RRCA
        ctx.cpu.a = 0x01;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x80);
        assert!(ctx.cpu.flag(FLAG_C));
    }

    #[test]
    fn test_cb_swap() {
        let mut ctx = setup_with_rom(&[
            0xCB, 0x37, // SWAP A
        ]);
        ctx.cpu.a = 0xF0;

        ctx.step();
        assert_eq!(ctx.cpu.a, 0x0F);
    }

    #[test]
    fn test_cb_bit() {
        let mut ctx = setup_with_rom(&[
            0xCB, 0x7F, // BIT 7, A
            0xCB, 0x47, // BIT 0, A
        ]);
        ctx.cpu.a = 0x80;

        ctx.step();
        assert!(!ctx.cpu.flag(FLAG_Z)); // Bit 7 is set

        ctx.step();
        assert!(ctx.cpu.flag(FLAG_Z)); // Bit 0 is not set
    }

    #[test]
    fn test_halt() {
        let mut ctx = setup_with_rom(&[0x76]); // HALT
        ctx.step();
        assert!(ctx.cpu.halted);
    }

    #[test]
    fn test_di_ei() {
        let mut ctx = setup_with_rom(&[
            0xF3, // DI
            0xFB, // EI
            0x00, // NOP
        ]);

        ctx.step();
        assert!(!ctx.cpu.ime);

        ctx.step();
        assert!(!ctx.cpu.ime); // Not yet enabled

        ctx.step();
        assert!(ctx.cpu.ime); // Now enabled after one instruction
    }

    #[test]
    fn test_ld_hl_n() {
        let mut ctx = setup_with_rom(&[
            0x21, 0x00, 0xC0, // LD HL, 0xC000
            0x36, 0x42, // LD (HL), 0x42
        ]);
        ctx.step(); // LD HL
        assert_eq!(ctx.cpu.hl(), 0xC000);

        ctx.step(); // LD (HL), 0x42
        // Read from WRAM at 0xC000
        let bus = MemoryBus::new(&mut ctx.memory, &mut ctx.timer, &mut ctx.joypad);
        assert_eq!(bus.read(0xC000), 0x42);
    }
}
