use crate::memory::Memory;

/// Interrupt controller that operates directly on memory's IF register (0xFF0F).
/// This ensures the CPU always sees the correct interrupt state.
pub struct InterruptController;

impl InterruptController {
    pub fn new() -> Self {
        InterruptController
    }

    pub fn request_vblank(&self, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg | 0x01);
    }

    pub fn request_lcd_stat(&self, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg | 0x02);
    }

    pub fn request_timer(&self, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg | 0x04);
    }

    /// Request serial interrupt. Currently unused as serial is not implemented.
    #[allow(dead_code)]
    pub fn request_serial(&self, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg | 0x08);
    }

    pub fn request_joypad(&self, memory: &mut Memory) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg | 0x10);
    }

    pub fn clear(&self, memory: &mut Memory, bit: u8) {
        let if_reg = memory.read_io_direct(0x0F);
        memory.write_io_direct(0x0F, if_reg & !(1 << bit));
    }
}

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_requests() {
        let ic = InterruptController::new();
        let mut mem = Memory::new();
        mem.write_io_direct(0x0F, 0x00);

        ic.request_vblank(&mut mem);
        assert_eq!(mem.read_io_direct(0x0F) & 0x01, 0x01);

        ic.request_lcd_stat(&mut mem);
        assert_eq!(mem.read_io_direct(0x0F) & 0x02, 0x02);

        ic.request_timer(&mut mem);
        assert_eq!(mem.read_io_direct(0x0F) & 0x04, 0x04);

        ic.request_serial(&mut mem);
        assert_eq!(mem.read_io_direct(0x0F) & 0x08, 0x08);

        ic.request_joypad(&mut mem);
        assert_eq!(mem.read_io_direct(0x0F) & 0x10, 0x10);
    }

    #[test]
    fn test_interrupt_clear() {
        let ic = InterruptController::new();
        let mut mem = Memory::new();
        mem.write_io_direct(0x0F, 0x1F); // All interrupts set

        ic.clear(&mut mem, 0); // Clear VBlank
        assert_eq!(mem.read_io_direct(0x0F), 0x1E);

        ic.clear(&mut mem, 2); // Clear Timer
        assert_eq!(mem.read_io_direct(0x0F), 0x1A);
    }
}
