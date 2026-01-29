//! Shared emulator core.
//!
//! [`GameBoyCore`] owns all emulator components and provides the main
//! `step_frame` loop, ROM loading, button input, and camera integration.

use crate::bus::MemoryBus;
use crate::cpu::Cpu;
use crate::interrupts::{Interrupt, InterruptController};
use crate::joypad::Joypad;
use crate::memory::Memory;
use crate::ppu::Ppu;
use crate::timer::Timer;

const CYCLES_PER_FRAME: u32 = 70224;
const FRAME_BUFFER_SIZE: usize = 160 * 144 * 4;
const CAMERA_BUFFER_SIZE: usize = 128 * 112 * 4;

pub(crate) struct DoubleBuffer<const N: usize> {
    buffers: [Box<[u8; N]>; 2],
    front: usize,
}

impl<const N: usize> DoubleBuffer<N> {
    pub fn new() -> Self {
        DoubleBuffer {
            buffers: [Box::new([0u8; N]), Box::new([0u8; N])],
            front: 0,
        }
    }

    #[inline]
    pub fn front(&self) -> &[u8; N] {
        &self.buffers[self.front]
    }

    #[inline]
    pub fn back_mut(&mut self) -> &mut [u8; N] {
        &mut self.buffers[1 - self.front]
    }

    #[inline]
    pub fn swap(&mut self) {
        self.front = 1 - self.front;
    }
}

pub(crate) struct GameBoyCore {
    pub(crate) cpu: Cpu,
    pub(crate) memory: Memory,
    pub(crate) ppu: Ppu,
    pub(crate) timer: Timer,
    pub(crate) interrupts: InterruptController,
    pub(crate) joypad: Joypad,
    pub(crate) frame_buffer: DoubleBuffer<FRAME_BUFFER_SIZE>,
    pub(crate) camera_live_buffer: DoubleBuffer<CAMERA_BUFFER_SIZE>,
    pub(crate) frame_count: u32,
    pub(crate) total_cycles: u64,
    pub(crate) instruction_count: u64,
}

impl GameBoyCore {
    pub(crate) fn new() -> Self {
        GameBoyCore {
            cpu: Cpu::new(),
            memory: Memory::new(),
            ppu: Ppu::new(),
            timer: Timer::new(),
            interrupts: InterruptController::new(),
            joypad: Joypad::new(),
            frame_buffer: DoubleBuffer::new(),
            camera_live_buffer: DoubleBuffer::new(),
            frame_count: 0,
            total_cycles: 0,
            instruction_count: 0,
        }
    }

    pub(crate) fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), &'static str> {
        self.memory.load_rom(rom_data)
    }

    /// Run one frame of emulation (~16.74ms of Game Boy time).
    /// Returns the number of instructions executed this frame.
    pub(crate) fn step_frame(&mut self) -> u32 {
        let mut cycles_elapsed: u32 = 0;
        let mut instructions_this_frame: u32 = 0;

        while cycles_elapsed < CYCLES_PER_FRAME {
            let cycles = {
                let mut bus =
                    MemoryBus::new(&mut self.memory, &mut self.timer, &mut self.joypad);
                self.cpu.step(&mut bus, &mut self.interrupts)
            };

            self.timer
                .tick(cycles, &mut self.memory, &self.interrupts);
            self.ppu
                .tick(cycles, &mut self.memory, &self.interrupts);

            cycles_elapsed += cycles;
            instructions_this_frame += 1;
            self.instruction_count += 1;
        }

        self.total_cycles += cycles_elapsed as u64;
        self.frame_count += 1;

        self.render_frame();
        instructions_this_frame
    }

    fn render_frame(&mut self) {
        let ppu_buffer = self.ppu.get_buffer();
        let palette = [0xFFu8, 0xAA, 0x55, 0x00];
        let back = self.frame_buffer.back_mut();

        for (i, &pixel) in ppu_buffer.iter().enumerate() {
            let gray = palette[(pixel & 0x03) as usize];
            let offset = i * 4;
            back[offset] = gray;
            back[offset + 1] = gray;
            back[offset + 2] = gray;
            back[offset + 3] = 255;
        }
        self.frame_buffer.swap();
    }

    pub(crate) fn set_button(&mut self, button: u8, pressed: bool) {
        if let Some(btn) = crate::joypad::Button::from_u8(button) {
            self.joypad.set_button(btn, pressed);
            if pressed {
                self.interrupts
                    .request(Interrupt::Joypad, &mut self.memory);
            }
        }
    }

    pub(crate) fn set_camera_image(&mut self, data: &[u8]) {
        self.memory.set_camera_image(data);
    }

    pub(crate) fn is_camera_cartridge(&self) -> bool {
        self.memory.get_mbc_type() == crate::memory::MbcType::PocketCamera
    }

    pub(crate) fn is_camera_ready(&self) -> bool {
        self.memory.is_camera_image_ready()
    }

    pub(crate) fn update_camera_live(&mut self) -> bool {
        if !self.memory.is_camera_capture_dirty() {
            return false;
        }
        self.memory.clear_camera_capture_dirty();

        let sram = self.memory.camera_capture_sram();
        let palette: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
        let buf = self.camera_live_buffer.back_mut();

        for tile_y in 0..14 {
            for tile_x in 0..16 {
                let tile_offset = (tile_y * 16 + tile_x) * 16;
                for row in 0..8 {
                    let low = sram[tile_offset + row * 2];
                    let high = sram[tile_offset + row * 2 + 1];
                    for col in 0..8 {
                        let bit = 7 - col;
                        let color_idx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);
                        let gray = palette[color_idx as usize];
                        let px = tile_x * 8 + col;
                        let py = tile_y * 8 + row;
                        let i = (py * 128 + px) * 4;
                        buf[i] = gray;
                        buf[i + 1] = gray;
                        buf[i + 2] = gray;
                        buf[i + 3] = 255;
                    }
                }
            }
        }
        self.camera_live_buffer.swap();
        true
    }

    pub(crate) fn decode_camera_photo(&self, slot: u8) -> Vec<u8> {
        self.memory.decode_camera_photo(slot)
    }
}
