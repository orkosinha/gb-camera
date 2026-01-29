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

/// Game Boy joypad buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
}

impl Button {
    /// Convert a raw `u8` button index to a `Button`.
    /// Returns `None` if the value is out of range.
    pub fn from_u8(value: u8) -> Option<Button> {
        match value {
            0 => Some(Button::A),
            1 => Some(Button::B),
            2 => Some(Button::Select),
            3 => Some(Button::Start),
            4 => Some(Button::Right),
            5 => Some(Button::Left),
            6 => Some(Button::Up),
            7 => Some(Button::Down),
            _ => None,
        }
    }
}

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

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        match button {
            Button::A => self.a = pressed,
            Button::B => self.b = pressed,
            Button::Select => self.select = pressed,
            Button::Start => self.start = pressed,
            Button::Right => self.right = pressed,
            Button::Left => self.left = pressed,
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
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
