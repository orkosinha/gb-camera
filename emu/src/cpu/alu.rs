//! ALU and rotate/shift operations.

use super::{Cpu, FLAG_C, FLAG_H, FLAG_N, FLAG_Z};

impl Cpu {
    // ALU operations
    #[inline]
    pub(super) fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (value & 0x0F) + 1 > 0x0F);
        result
    }

    #[inline]
    pub(super) fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (value & 0x0F) == 0);
        result
    }

    #[inline]
    pub(super) fn add(&mut self, value: u8) {
        let result = self.a as u16 + value as u16;
        self.set_flag(FLAG_Z, (result & 0xFF) == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0x0F) + (value & 0x0F) > 0x0F);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
    }

    #[inline]
    pub(super) fn adc(&mut self, value: u8) {
        let carry = if self.flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as u16 + value as u16 + carry as u16;
        self.set_flag(FLAG_Z, (result & 0xFF) == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0x0F) + (value & 0x0F) + carry > 0x0F);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
    }

    #[inline]
    pub(super) fn sub(&mut self, value: u8) {
        let result = self.a.wrapping_sub(value);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F));
        self.set_flag(FLAG_C, self.a < value);
        self.a = result;
    }

    #[inline]
    pub(super) fn sbc(&mut self, value: u8) {
        let carry = if self.flag(FLAG_C) { 1u8 } else { 0 };
        let result = self.a.wrapping_sub(value).wrapping_sub(carry);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F) + carry);
        self.set_flag(FLAG_C, (self.a as u16) < (value as u16) + (carry as u16));
        self.a = result;
    }

    #[inline]
    pub(super) fn and(&mut self, value: u8) {
        self.a &= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, true);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    pub(super) fn xor(&mut self, value: u8) {
        self.a ^= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    pub(super) fn or(&mut self, value: u8) {
        self.a |= value;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    #[inline]
    pub(super) fn cp(&mut self, value: u8) {
        self.set_flag(FLAG_Z, self.a == value);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0x0F) < (value & 0x0F));
        self.set_flag(FLAG_C, self.a < value);
    }

    #[inline]
    pub(super) fn add_hl(&mut self, value: u16) {
        let hl = self.hl();
        let result = hl.wrapping_add(value);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF);
        self.set_flag(FLAG_C, hl > 0xFFFF - value);
        self.set_hl(result);
    }

    #[inline]
    pub(super) fn daa(&mut self) {
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
    pub(super) fn rlc(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = (value << 1) | carry;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    pub(super) fn rrc(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (carry << 7);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    pub(super) fn rl(&mut self, value: u8) -> u8 {
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
    pub(super) fn rr(&mut self, value: u8) -> u8 {
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
    pub(super) fn sla(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = value << 1;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    pub(super) fn sra(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (value & 0x80);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    pub(super) fn srl(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = value >> 1;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry == 1);
        result
    }

    #[inline]
    pub(super) fn swap(&mut self, value: u8) -> u8 {
        let result = value.rotate_left(4);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
        result
    }
}
