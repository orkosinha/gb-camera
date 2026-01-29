//! Sharp LR35902 CPU emulation.
//!
//! Implements the full instruction set including CB-prefixed opcodes,
//! interrupt handling, and the HALT state. Each `step` call executes
//! one instruction and returns the number of T-cycles consumed.

use std::fmt;

use crate::bus::MemoryBus;
use crate::interrupts::{Interrupt, InterruptController};
use crate::memory::io;
use crate::log::LogCategory;
use crate::log_info;

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

    fn execute(&mut self, opcode: u8, bus: &mut MemoryBus) -> u32 {
        match opcode {
            0x00 => 4, // NOP

            // LD r, n (8-bit immediate loads)
            0x06 => {
                self.b = self.fetch(bus);
                8
            }
            0x0E => {
                self.c = self.fetch(bus);
                8
            }
            0x16 => {
                self.d = self.fetch(bus);
                8
            }
            0x1E => {
                self.e = self.fetch(bus);
                8
            }
            0x26 => {
                self.h = self.fetch(bus);
                8
            }
            0x2E => {
                self.l = self.fetch(bus);
                8
            }
            0x36 => {
                let n = self.fetch(bus);
                bus.write(self.hl(), n);
                12
            } // LD (HL), n
            0x3E => {
                self.a = self.fetch(bus);
                8
            }

            // LD r, r (register to register)
            0x40 => 4, // LD B, B
            0x41 => {
                self.b = self.c;
                4
            }
            0x42 => {
                self.b = self.d;
                4
            }
            0x43 => {
                self.b = self.e;
                4
            }
            0x44 => {
                self.b = self.h;
                4
            }
            0x45 => {
                self.b = self.l;
                4
            }
            0x46 => {
                self.b = bus.read(self.hl());
                8
            }
            0x47 => {
                self.b = self.a;
                4
            }

            0x48 => {
                self.c = self.b;
                4
            }
            0x49 => 4, // LD C, C
            0x4A => {
                self.c = self.d;
                4
            }
            0x4B => {
                self.c = self.e;
                4
            }
            0x4C => {
                self.c = self.h;
                4
            }
            0x4D => {
                self.c = self.l;
                4
            }
            0x4E => {
                self.c = bus.read(self.hl());
                8
            }
            0x4F => {
                self.c = self.a;
                4
            }

            0x50 => {
                self.d = self.b;
                4
            }
            0x51 => {
                self.d = self.c;
                4
            }
            0x52 => 4, // LD D, D
            0x53 => {
                self.d = self.e;
                4
            }
            0x54 => {
                self.d = self.h;
                4
            }
            0x55 => {
                self.d = self.l;
                4
            }
            0x56 => {
                self.d = bus.read(self.hl());
                8
            }
            0x57 => {
                self.d = self.a;
                4
            }

            0x58 => {
                self.e = self.b;
                4
            }
            0x59 => {
                self.e = self.c;
                4
            }
            0x5A => {
                self.e = self.d;
                4
            }
            0x5B => 4, // LD E, E
            0x5C => {
                self.e = self.h;
                4
            }
            0x5D => {
                self.e = self.l;
                4
            }
            0x5E => {
                self.e = bus.read(self.hl());
                8
            }
            0x5F => {
                self.e = self.a;
                4
            }

            0x60 => {
                self.h = self.b;
                4
            }
            0x61 => {
                self.h = self.c;
                4
            }
            0x62 => {
                self.h = self.d;
                4
            }
            0x63 => {
                self.h = self.e;
                4
            }
            0x64 => 4, // LD H, H
            0x65 => {
                self.h = self.l;
                4
            }
            0x66 => {
                self.h = bus.read(self.hl());
                8
            }
            0x67 => {
                self.h = self.a;
                4
            }

            0x68 => {
                self.l = self.b;
                4
            }
            0x69 => {
                self.l = self.c;
                4
            }
            0x6A => {
                self.l = self.d;
                4
            }
            0x6B => {
                self.l = self.e;
                4
            }
            0x6C => {
                self.l = self.h;
                4
            }
            0x6D => 4, // LD L, L
            0x6E => {
                self.l = bus.read(self.hl());
                8
            }
            0x6F => {
                self.l = self.a;
                4
            }

            0x70 => {
                bus.write(self.hl(), self.b);
                8
            }
            0x71 => {
                bus.write(self.hl(), self.c);
                8
            }
            0x72 => {
                bus.write(self.hl(), self.d);
                8
            }
            0x73 => {
                bus.write(self.hl(), self.e);
                8
            }
            0x74 => {
                bus.write(self.hl(), self.h);
                8
            }
            0x75 => {
                bus.write(self.hl(), self.l);
                8
            }
            0x77 => {
                bus.write(self.hl(), self.a);
                8
            }

            0x78 => {
                self.a = self.b;
                4
            }
            0x79 => {
                self.a = self.c;
                4
            }
            0x7A => {
                self.a = self.d;
                4
            }
            0x7B => {
                self.a = self.e;
                4
            }
            0x7C => {
                self.a = self.h;
                4
            }
            0x7D => {
                self.a = self.l;
                4
            }
            0x7E => {
                self.a = bus.read(self.hl());
                8
            }
            0x7F => 4, // LD A, A

            // 16-bit loads
            0x01 => {
                let v = self.fetch_word(bus);
                self.set_bc(v);
                12
            }
            0x11 => {
                let v = self.fetch_word(bus);
                self.set_de(v);
                12
            }
            0x21 => {
                let v = self.fetch_word(bus);
                self.set_hl(v);
                12
            }
            0x31 => {
                self.sp = self.fetch_word(bus);
                12
            }

            // LD (nn), SP
            0x08 => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.sp as u8);
                bus.write(addr.wrapping_add(1), (self.sp >> 8) as u8);
                20
            }

            // LD A, (BC/DE)
            0x0A => {
                self.a = bus.read(self.bc());
                8
            }
            0x1A => {
                self.a = bus.read(self.de());
                8
            }

            // LD (BC/DE), A
            0x02 => {
                bus.write(self.bc(), self.a);
                8
            }
            0x12 => {
                bus.write(self.de(), self.a);
                8
            }

            // LD A, (HL+/-) and LD (HL+/-), A
            0x22 => {
                let hl = self.hl();
                bus.write(hl, self.a);
                self.set_hl(hl.wrapping_add(1));
                8
            }
            0x2A => {
                let hl = self.hl();
                self.a = bus.read(hl);
                self.set_hl(hl.wrapping_add(1));
                8
            }
            0x32 => {
                let hl = self.hl();
                bus.write(hl, self.a);
                self.set_hl(hl.wrapping_sub(1));
                8
            }
            0x3A => {
                let hl = self.hl();
                self.a = bus.read(hl);
                self.set_hl(hl.wrapping_sub(1));
                8
            }

            // LD A, (nn) and LD (nn), A
            0xEA => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.a);
                16
            }
            0xFA => {
                let addr = self.fetch_word(bus);
                self.a = bus.read(addr);
                16
            }

            // LDH A, (n) and LDH (n), A
            0xE0 => {
                let n = self.fetch(bus);
                bus.write(0xFF00 | n as u16, self.a);
                12
            }
            0xF0 => {
                let n = self.fetch(bus);
                self.a = bus.read(0xFF00 | n as u16);
                12
            }

            // LDH A, (C) and LDH (C), A
            0xE2 => {
                bus.write(0xFF00 | self.c as u16, self.a);
                8
            }
            0xF2 => {
                self.a = bus.read(0xFF00 | self.c as u16);
                8
            }

            // PUSH/POP
            0xC5 => {
                self.push_word(bus, self.bc());
                16
            }
            0xD5 => {
                self.push_word(bus, self.de());
                16
            }
            0xE5 => {
                self.push_word(bus, self.hl());
                16
            }
            0xF5 => {
                self.push_word(bus, self.af());
                16
            }
            0xC1 => {
                let v = self.pop_word(bus);
                self.set_bc(v);
                12
            }
            0xD1 => {
                let v = self.pop_word(bus);
                self.set_de(v);
                12
            }
            0xE1 => {
                let v = self.pop_word(bus);
                self.set_hl(v);
                12
            }
            0xF1 => {
                let v = self.pop_word(bus);
                self.set_af(v);
                12
            }

            // INC/DEC 8-bit
            0x04 => {
                self.b = self.inc(self.b);
                4
            }
            0x0C => {
                self.c = self.inc(self.c);
                4
            }
            0x14 => {
                self.d = self.inc(self.d);
                4
            }
            0x1C => {
                self.e = self.inc(self.e);
                4
            }
            0x24 => {
                self.h = self.inc(self.h);
                4
            }
            0x2C => {
                self.l = self.inc(self.l);
                4
            }
            0x34 => {
                let v = self.inc(bus.read(self.hl()));
                bus.write(self.hl(), v);
                12
            }
            0x3C => {
                self.a = self.inc(self.a);
                4
            }

            0x05 => {
                self.b = self.dec(self.b);
                4
            }
            0x0D => {
                self.c = self.dec(self.c);
                4
            }
            0x15 => {
                self.d = self.dec(self.d);
                4
            }
            0x1D => {
                self.e = self.dec(self.e);
                4
            }
            0x25 => {
                self.h = self.dec(self.h);
                4
            }
            0x2D => {
                self.l = self.dec(self.l);
                4
            }
            0x35 => {
                let v = self.dec(bus.read(self.hl()));
                bus.write(self.hl(), v);
                12
            }
            0x3D => {
                self.a = self.dec(self.a);
                4
            }

            // INC/DEC 16-bit
            0x03 => {
                self.set_bc(self.bc().wrapping_add(1));
                8
            }
            0x13 => {
                self.set_de(self.de().wrapping_add(1));
                8
            }
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                8
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                8
            }
            0x0B => {
                self.set_bc(self.bc().wrapping_sub(1));
                8
            }
            0x1B => {
                self.set_de(self.de().wrapping_sub(1));
                8
            }
            0x2B => {
                self.set_hl(self.hl().wrapping_sub(1));
                8
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                8
            }

            // ADD A, r
            0x80 => {
                self.add(self.b);
                4
            }
            0x81 => {
                self.add(self.c);
                4
            }
            0x82 => {
                self.add(self.d);
                4
            }
            0x83 => {
                self.add(self.e);
                4
            }
            0x84 => {
                self.add(self.h);
                4
            }
            0x85 => {
                self.add(self.l);
                4
            }
            0x86 => {
                self.add(bus.read(self.hl()));
                8
            }
            0x87 => {
                self.add(self.a);
                4
            }
            0xC6 => {
                let n = self.fetch(bus);
                self.add(n);
                8
            }

            // ADC A, r
            0x88 => {
                self.adc(self.b);
                4
            }
            0x89 => {
                self.adc(self.c);
                4
            }
            0x8A => {
                self.adc(self.d);
                4
            }
            0x8B => {
                self.adc(self.e);
                4
            }
            0x8C => {
                self.adc(self.h);
                4
            }
            0x8D => {
                self.adc(self.l);
                4
            }
            0x8E => {
                self.adc(bus.read(self.hl()));
                8
            }
            0x8F => {
                self.adc(self.a);
                4
            }
            0xCE => {
                let n = self.fetch(bus);
                self.adc(n);
                8
            }

            // SUB A, r
            0x90 => {
                self.sub(self.b);
                4
            }
            0x91 => {
                self.sub(self.c);
                4
            }
            0x92 => {
                self.sub(self.d);
                4
            }
            0x93 => {
                self.sub(self.e);
                4
            }
            0x94 => {
                self.sub(self.h);
                4
            }
            0x95 => {
                self.sub(self.l);
                4
            }
            0x96 => {
                self.sub(bus.read(self.hl()));
                8
            }
            0x97 => {
                self.sub(self.a);
                4
            }
            0xD6 => {
                let n = self.fetch(bus);
                self.sub(n);
                8
            }

            // SBC A, r
            0x98 => {
                self.sbc(self.b);
                4
            }
            0x99 => {
                self.sbc(self.c);
                4
            }
            0x9A => {
                self.sbc(self.d);
                4
            }
            0x9B => {
                self.sbc(self.e);
                4
            }
            0x9C => {
                self.sbc(self.h);
                4
            }
            0x9D => {
                self.sbc(self.l);
                4
            }
            0x9E => {
                self.sbc(bus.read(self.hl()));
                8
            }
            0x9F => {
                self.sbc(self.a);
                4
            }
            0xDE => {
                let n = self.fetch(bus);
                self.sbc(n);
                8
            }

            // AND A, r
            0xA0 => {
                self.and(self.b);
                4
            }
            0xA1 => {
                self.and(self.c);
                4
            }
            0xA2 => {
                self.and(self.d);
                4
            }
            0xA3 => {
                self.and(self.e);
                4
            }
            0xA4 => {
                self.and(self.h);
                4
            }
            0xA5 => {
                self.and(self.l);
                4
            }
            0xA6 => {
                self.and(bus.read(self.hl()));
                8
            }
            0xA7 => {
                self.and(self.a);
                4
            }
            0xE6 => {
                let n = self.fetch(bus);
                self.and(n);
                8
            }

            // XOR A, r
            0xA8 => {
                self.xor(self.b);
                4
            }
            0xA9 => {
                self.xor(self.c);
                4
            }
            0xAA => {
                self.xor(self.d);
                4
            }
            0xAB => {
                self.xor(self.e);
                4
            }
            0xAC => {
                self.xor(self.h);
                4
            }
            0xAD => {
                self.xor(self.l);
                4
            }
            0xAE => {
                self.xor(bus.read(self.hl()));
                8
            }
            0xAF => {
                self.xor(self.a);
                4
            }
            0xEE => {
                let n = self.fetch(bus);
                self.xor(n);
                8
            }

            // OR A, r
            0xB0 => {
                self.or(self.b);
                4
            }
            0xB1 => {
                self.or(self.c);
                4
            }
            0xB2 => {
                self.or(self.d);
                4
            }
            0xB3 => {
                self.or(self.e);
                4
            }
            0xB4 => {
                self.or(self.h);
                4
            }
            0xB5 => {
                self.or(self.l);
                4
            }
            0xB6 => {
                self.or(bus.read(self.hl()));
                8
            }
            0xB7 => {
                self.or(self.a);
                4
            }
            0xF6 => {
                let n = self.fetch(bus);
                self.or(n);
                8
            }

            // CP A, r
            0xB8 => {
                self.cp(self.b);
                4
            }
            0xB9 => {
                self.cp(self.c);
                4
            }
            0xBA => {
                self.cp(self.d);
                4
            }
            0xBB => {
                self.cp(self.e);
                4
            }
            0xBC => {
                self.cp(self.h);
                4
            }
            0xBD => {
                self.cp(self.l);
                4
            }
            0xBE => {
                self.cp(bus.read(self.hl()));
                8
            }
            0xBF => {
                self.cp(self.a);
                4
            }
            0xFE => {
                let n = self.fetch(bus);
                self.cp(n);
                8
            }

            // Jumps
            0xC3 => {
                self.pc = self.fetch_word(bus);
                16
            }
            0xC2 => {
                let addr = self.fetch_word(bus);
                if !self.flag(FLAG_Z) {
                    self.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xCA => {
                let addr = self.fetch_word(bus);
                if self.flag(FLAG_Z) {
                    self.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xD2 => {
                let addr = self.fetch_word(bus);
                if !self.flag(FLAG_C) {
                    self.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xDA => {
                let addr = self.fetch_word(bus);
                if self.flag(FLAG_C) {
                    self.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xE9 => {
                self.pc = self.hl();
                4
            }

            // JR (relative jumps)
            0x18 => {
                let offset = self.fetch(bus) as i8;
                self.pc = self.pc.wrapping_add(offset as u16);
                12
            }
            0x20 => {
                let offset = self.fetch(bus) as i8;
                if !self.flag(FLAG_Z) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }
            0x28 => {
                let offset = self.fetch(bus) as i8;
                if self.flag(FLAG_Z) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }
            0x30 => {
                let offset = self.fetch(bus) as i8;
                if !self.flag(FLAG_C) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }
            0x38 => {
                let offset = self.fetch(bus) as i8;
                if self.flag(FLAG_C) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }

            // CALL
            0xCD => {
                let addr = self.fetch_word(bus);
                self.push_word(bus, self.pc);
                self.pc = addr;
                24
            }
            0xC4 => {
                let addr = self.fetch_word(bus);
                if !self.flag(FLAG_Z) {
                    self.push_word(bus, self.pc);
                    self.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xCC => {
                let addr = self.fetch_word(bus);
                if self.flag(FLAG_Z) {
                    self.push_word(bus, self.pc);
                    self.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xD4 => {
                let addr = self.fetch_word(bus);
                if !self.flag(FLAG_C) {
                    self.push_word(bus, self.pc);
                    self.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xDC => {
                let addr = self.fetch_word(bus);
                if self.flag(FLAG_C) {
                    self.push_word(bus, self.pc);
                    self.pc = addr;
                    24
                } else {
                    12
                }
            }

            // RET
            0xC9 => {
                self.pc = self.pop_word(bus);
                16
            }
            0xC0 => {
                if !self.flag(FLAG_Z) {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xC8 => {
                if self.flag(FLAG_Z) {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xD0 => {
                if !self.flag(FLAG_C) {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xD8 => {
                if self.flag(FLAG_C) {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xD9 => {
                self.ime = true;
                self.pc = self.pop_word(bus);
                16
            } // RETI

            // RST
            0xC7 => {
                self.push_word(bus, self.pc);
                self.pc = 0x00;
                16
            }
            0xCF => {
                self.push_word(bus, self.pc);
                self.pc = 0x08;
                16
            }
            0xD7 => {
                self.push_word(bus, self.pc);
                self.pc = 0x10;
                16
            }
            0xDF => {
                self.push_word(bus, self.pc);
                self.pc = 0x18;
                16
            }
            0xE7 => {
                self.push_word(bus, self.pc);
                self.pc = 0x20;
                16
            }
            0xEF => {
                self.push_word(bus, self.pc);
                self.pc = 0x28;
                16
            }
            0xF7 => {
                self.push_word(bus, self.pc);
                self.pc = 0x30;
                16
            }
            0xFF => {
                self.push_word(bus, self.pc);
                self.pc = 0x38;
                16
            }

            // Misc
            0x76 => {
                self.halted = true;
                4
            } // HALT
            0x10 => {
                self.fetch(bus);
                4
            } // STOP (skip next byte)
            0xF3 => {
                self.ime = false;
                4
            } // DI
            0xFB => {
                self.ime_pending = true;
                4
            } // EI

            0x27 => {
                self.daa();
                4
            } // DAA
            0x2F => {
                self.a = !self.a;
                self.set_flag(FLAG_N, true);
                self.set_flag(FLAG_H, true);
                4
            } // CPL
            0x37 => {
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, true);
                4
            } // SCF
            0x3F => {
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, !self.flag(FLAG_C));
                4
            } // CCF

            // Rotates
            0x07 => {
                self.a = self.rlc(self.a);
                self.set_flag(FLAG_Z, false);
                4
            } // RLCA
            0x0F => {
                self.a = self.rrc(self.a);
                self.set_flag(FLAG_Z, false);
                4
            } // RRCA
            0x17 => {
                self.a = self.rl(self.a);
                self.set_flag(FLAG_Z, false);
                4
            } // RLA
            0x1F => {
                self.a = self.rr(self.a);
                self.set_flag(FLAG_Z, false);
                4
            } // RRA

            // ADD HL, rr
            0x09 => {
                self.add_hl(self.bc());
                8
            }
            0x19 => {
                self.add_hl(self.de());
                8
            }
            0x29 => {
                self.add_hl(self.hl());
                8
            }
            0x39 => {
                self.add_hl(self.sp);
                8
            }

            // ADD SP, n
            0xE8 => {
                let n = self.fetch(bus) as i8 as i16 as u16;
                let result = self.sp.wrapping_add(n);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, (self.sp & 0x0F) + (n & 0x0F) > 0x0F);
                self.set_flag(FLAG_C, (self.sp & 0xFF) + (n & 0xFF) > 0xFF);
                self.sp = result;
                16
            }

            // LD HL, SP+n
            0xF8 => {
                let n = self.fetch(bus) as i8 as i16 as u16;
                let result = self.sp.wrapping_add(n);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, (self.sp & 0x0F) + (n & 0x0F) > 0x0F);
                self.set_flag(FLAG_C, (self.sp & 0xFF) + (n & 0xFF) > 0xFF);
                self.set_hl(result);
                12
            }

            // LD SP, HL
            0xF9 => {
                self.sp = self.hl();
                8
            }

            // CB prefix
            0xCB => {
                let cb_opcode = self.fetch(bus);
                self.execute_cb(cb_opcode, bus)
            }

            _ => {
                // Unimplemented opcode
                panic!(
                    "Unimplemented opcode: 0x{:02X} at PC: 0x{:04X}",
                    opcode,
                    self.pc.wrapping_sub(1)
                );
            }
        }
    }

    fn execute_cb(&mut self, opcode: u8, bus: &mut MemoryBus) -> u32 {
        let reg_idx = opcode & 0x07;
        let op_type = opcode >> 3;

        let value = self.get_reg(reg_idx, bus);
        let is_hl = reg_idx == 6;
        let base_cycles = if is_hl { 16 } else { 8 };

        let result = match op_type {
            0x00 => self.rlc(value),  // RLC
            0x01 => self.rrc(value),  // RRC
            0x02 => self.rl(value),   // RL
            0x03 => self.rr(value),   // RR
            0x04 => self.sla(value),  // SLA
            0x05 => self.sra(value),  // SRA
            0x06 => self.swap(value), // SWAP
            0x07 => self.srl(value),  // SRL
            0x08..=0x0F => {
                // BIT
                let bit = op_type - 0x08;
                self.set_flag(FLAG_Z, (value & (1 << bit)) == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, true);
                return if is_hl { 12 } else { 8 };
            }
            0x10..=0x17 => value & !(1 << (op_type - 0x10)), // RES
            0x18..=0x1F => value | (1 << (op_type - 0x18)),  // SET
            _ => value,
        };

        self.set_reg(reg_idx, result, bus);
        base_cycles
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

    // ALU operations
    #[inline]
    fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (value & 0x0F) + 1 > 0x0F);
        result
    }

    #[inline]
    fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (value & 0x0F) == 0);
        result
    }

    #[inline]
    fn add(&mut self, value: u8) {
        let result = self.a as u16 + value as u16;
        self.set_flag(FLAG_Z, (result & 0xFF) == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0x0F) + (value & 0x0F) > 0x0F);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
    }

    #[inline]
    fn adc(&mut self, value: u8) {
        let carry = if self.flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as u16 + value as u16 + carry as u16;
        self.set_flag(FLAG_Z, (result & 0xFF) == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0x0F) + (value & 0x0F) + carry > 0x0F);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
    }

    #[inline]
    fn sub(&mut self, value: u8) {
        let result = self.a.wrapping_sub(value);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F));
        self.set_flag(FLAG_C, self.a < value);
        self.a = result;
    }

    #[inline]
    fn sbc(&mut self, value: u8) {
        let carry = if self.flag(FLAG_C) { 1u8 } else { 0 };
        let result = self.a.wrapping_sub(value).wrapping_sub(carry);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F) + carry);
        self.set_flag(FLAG_C, (self.a as u16) < (value as u16) + (carry as u16));
        self.a = result;
    }

    #[inline]
    fn and(&mut self, value: u8) {
        self.a &= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, true);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    fn xor(&mut self, value: u8) {
        self.a ^= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    fn or(&mut self, value: u8) {
        self.a |= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    fn cp(&mut self, value: u8) {
        self.set_flag(FLAG_Z, self.a == value);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F));
        self.set_flag(FLAG_C, self.a < value);
    }

    #[inline]
    fn add_hl(&mut self, value: u16) {
        let hl = self.hl();
        let result = hl.wrapping_add(value);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF);
        self.set_flag(FLAG_C, hl > 0xFFFF - value);
        self.set_hl(result);
    }

    #[inline]
    fn daa(&mut self) {
        let mut adjust = 0u8;
        let mut carry = false;

        if self.flag(FLAG_N) {
            if self.flag(FLAG_C) {
                adjust |= 0x60;
                carry = true;
            }
            if self.flag(FLAG_H) {
                adjust |= 0x06;
            }
            self.a = self.a.wrapping_sub(adjust);
        } else {
            if self.flag(FLAG_C) || self.a > 0x99 {
                adjust |= 0x60;
                carry = true;
            }
            if self.flag(FLAG_H) || (self.a & 0x0F) > 0x09 {
                adjust |= 0x06;
            }
            self.a = self.a.wrapping_add(adjust);
        }

        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry);
    }

    // Rotate/Shift operations
    #[inline]
    fn rlc(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = (value << 1) | carry;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    fn rrc(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (carry << 7);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    fn rl(&mut self, value: u8) -> u8 {
        let old_carry = if self.flag(FLAG_C) { 1 } else { 0 };
        let new_carry = value >> 7;
        let result = (value << 1) | old_carry;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, new_carry == 1);
        result
    }

    #[inline]
    fn rr(&mut self, value: u8) -> u8 {
        let old_carry = if self.flag(FLAG_C) { 1 } else { 0 };
        let new_carry = value & 1;
        let result = (value >> 1) | (old_carry << 7);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, new_carry == 1);
        result
    }

    #[inline]
    fn sla(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = value << 1;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    fn sra(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (value & 0x80);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    fn srl(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = value >> 1;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    fn swap(&mut self, value: u8) -> u8 {
        let result = value.rotate_left(4);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
        result
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
