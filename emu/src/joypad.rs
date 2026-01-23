pub struct Joypad {
    // Button states (active low in hardware, but we track as true = pressed)
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,

    // Selection register (0xFF00 bits 4-5)
    select_buttons: bool,
    select_dpad: bool,
}

// Button indices for set_button
pub const BUTTON_A: u8 = 0;
pub const BUTTON_B: u8 = 1;
pub const BUTTON_SELECT: u8 = 2;
pub const BUTTON_START: u8 = 3;
pub const BUTTON_RIGHT: u8 = 4;
pub const BUTTON_LEFT: u8 = 5;
pub const BUTTON_UP: u8 = 6;
pub const BUTTON_DOWN: u8 = 7;

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            a: false,
            b: false,
            select: false,
            start: false,
            right: false,
            left: false,
            up: false,
            down: false,
            select_buttons: false,
            select_dpad: false,
        }
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        match button {
            BUTTON_A => self.a = pressed,
            BUTTON_B => self.b = pressed,
            BUTTON_SELECT => self.select = pressed,
            BUTTON_START => self.start = pressed,
            BUTTON_RIGHT => self.right = pressed,
            BUTTON_LEFT => self.left = pressed,
            BUTTON_UP => self.up = pressed,
            BUTTON_DOWN => self.down = pressed,
            _ => {}
        }
    }

    /// Read the joypad register (0xFF00). Returns button states based on selection bits.
    pub fn read(&self) -> u8 {
        let mut result = 0xCF; // Bits 6-7 always 1, bits 4-5 depend on selection

        if !self.select_buttons {
            result |= 0x20;
        }
        if !self.select_dpad {
            result |= 0x10;
        }

        // Lower 4 bits: active low (0 = pressed)
        if self.select_buttons {
            if self.a {
                result &= !0x01;
            }
            if self.b {
                result &= !0x02;
            }
            if self.select {
                result &= !0x04;
            }
            if self.start {
                result &= !0x08;
            }
        }

        if self.select_dpad {
            if self.right {
                result &= !0x01;
            }
            if self.left {
                result &= !0x02;
            }
            if self.up {
                result &= !0x04;
            }
            if self.down {
                result &= !0x08;
            }
        }

        result
    }

    /// Write to the joypad register (0xFF00) to select button/d-pad reading mode.
    pub fn write(&mut self, value: u8) {
        // Bits 4-5 select which buttons to read
        self.select_buttons = value & 0x20 == 0;
        self.select_dpad = value & 0x10 == 0;
    }
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_buttons_pressed() {
        let mut joypad = Joypad::new();
        joypad.select_buttons = true;
        joypad.select_dpad = false;

        // No buttons pressed, lower nibble should be 0xF (all high)
        let result = joypad.read();
        assert_eq!(result & 0x0F, 0x0F);
    }

    #[test]
    fn test_a_button_pressed() {
        let mut joypad = Joypad::new();
        joypad.select_buttons = true;
        joypad.select_dpad = false;
        joypad.a = true;

        let result = joypad.read();
        assert_eq!(result & 0x01, 0x00); // A is bit 0, should be low
    }

    #[test]
    fn test_dpad_pressed() {
        let mut joypad = Joypad::new();
        joypad.select_buttons = false;
        joypad.select_dpad = true;
        joypad.up = true;
        joypad.right = true;

        let result = joypad.read();
        assert_eq!(result & 0x01, 0x00); // Right
        assert_eq!(result & 0x04, 0x00); // Up
    }
}
