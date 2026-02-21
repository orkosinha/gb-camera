//! Opcode decode tables.

use super::{Cpu, FLAG_C, FLAG_H, FLAG_N, FLAG_Z};
use crate::bus::MemoryBus;

impl Cpu {
    pub(super) fn execute(&mut self, opcode: u8, bus: &mut MemoryBus) -> u32 {
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
                self.fetch(bus); // consume the mandatory 0x00 operand
                // In GBC mode, STOP with KEY1 armed triggers CPU speed switch
                if bus.read_io_direct(crate::memory::io::KEY1) & 0x01 != 0 {
                    bus.memory_mut().toggle_double_speed();
                } else {
                    self.halted = true;
                }
                4
            } // STOP / speed switch
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
}
