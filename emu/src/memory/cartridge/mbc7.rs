//! MBC7 cartridge — ROM + ADXL202E accelerometer + 93LC56 EEPROM.
//!
//! Cartridge type 0x22. Used by Kirby's Tilt 'n' Tumble (and the unreleased Command Master).
//!
//! ROM register layout:
//!   0x0000-0x1FFF  RAM gate 1  — write 0x0A to open
//!   0x2000-0x3FFF  ROM bank number
//!   0x4000-0x5FFF  RAM gate 2  — write 0x40 to open (both gates must be open for RAM access)
//!
//! RAM address layout (0xA000-0xBFFF). Address bits 4-7 select the register; bits 0-3 ignored.
//! Both RAM gates must be open for any access.
//!   reg 0  (0xA000): Latch step 1 — write 0x55 to erase latched data (returns 0xFF on read)
//!   reg 1  (0xA010): Latch step 2 — write 0xAA to capture current reading (returns 0xFF on read)
//!   reg 2  (0xA020): X-axis low  byte (LSB of 16-bit reading; center ≈ 0x81D0 when flat)
//!   reg 3  (0xA030): X-axis high byte
//!   reg 4  (0xA040): Y-axis low  byte
//!   reg 5  (0xA050): Y-axis high byte
//!   reg 6  (0xA060): Z-axis low  byte (always 0x00)
//!   reg 7  (0xA070): Z-axis high byte (always 0xFF)
//!   reg 8+ (0xA080): 93LC56 EEPROM bit-serial interface (mirrored across rest of range)

use super::{Cartridge, MbcType};

const ROM_BANK_SIZE: usize = 0x4000;

/// Neutral accelerometer reading (device held flat). Hardware-measured value.
const ACCEL_CENTER: i32 = 0x81D0;

// ── EEPROM ───────────────────────────────────────────────────────────────────

/// 93LC56B EEPROM — 128 × 16-bit words (x16 organisation), Microwire serial interface.
///
/// The byte register at 0xA008 maps to four signal lines:
///   Bit 7 (R/W): CS  — chip select, active high
///   Bit 6 (R/W): CLK — clock
///   Bit 1 (W):   DI  — serial data in  (master → chip)
///   Bit 0 (R):   DO  — serial data out (chip → master)
///
/// Each transaction: CS↑ → {start(1), opcode(2), address(7)} → [data(16)] → CS↓.
/// Data is MSB-first; DO transitions on CLK rising edge.
pub struct Eeprom93lc56 {
    /// 256-byte backing store, little-endian 16-bit words.
    data: [u8; 256],
    write_enabled: bool,

    // Pin state (last written values)
    cs: bool,
    clk: bool,
    di: bool,
    do_bit: bool,

    // Internal state machine
    state: EepromState,
    in_bits: u32, // bits received so far (shifted left)
    in_count: u8, // number of bits received (counting from first 1-bit)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EepromState {
    Idle,
    /// Collecting start bit + 2-bit opcode + 7-bit address (= 10 bits).
    Receiving,
    /// Shifting a 16-bit word out to the master. `word` is the value; `sent` = bits output so far.
    Reading { word: u16, sent: u8 },
    /// Collecting 16-bit write data for a single address.
    Writing { addr: u8, data: u16, received: u8 },
    /// Collecting 16-bit write data for WRAL (write-all).
    WritingAll { data: u16, received: u8 },
}

impl Eeprom93lc56 {
    pub fn new() -> Self {
        Eeprom93lc56 {
            data: [0xFF; 256], // erased / blank state
            write_enabled: false,
            cs: false,
            clk: false,
            di: false,
            do_bit: true, // high = ready
            state: EepromState::Idle,
            in_bits: 0,
            in_count: 0,
        }
    }

    /// Read the register byte (reconstructed from current pin state).
    pub fn read(&self) -> u8 {
        let cs  = if self.cs     { 0x80 } else { 0 };
        let clk = if self.clk    { 0x40 } else { 0 };
        let di  = if self.di     { 0x02 } else { 0 };
        let do_ = if self.do_bit { 0x01 } else { 0 };
        cs | clk | di | do_
    }

    /// Write the register byte (updates CS/CLK/DI and advances state machine).
    pub fn write(&mut self, value: u8) {
        let new_cs  = value & 0x80 != 0;
        let new_clk = value & 0x40 != 0;
        let di      = value & 0x02 != 0;
        self.di = di;

        // CS falling edge → reset
        if self.cs && !new_cs {
            self.state    = EepromState::Idle;
            self.in_bits  = 0;
            self.in_count = 0;
            self.do_bit   = true;
        }

        // CS rising edge → begin transaction
        if !self.cs && new_cs {
            self.state    = EepromState::Receiving;
            self.in_bits  = 0;
            self.in_count = 0;
        }

        let rising = new_cs && !self.clk && new_clk;
        self.cs  = new_cs;
        self.clk = new_clk;

        if !rising {
            return;
        }

        // Process rising CLK edge
        match self.state {
            EepromState::Idle => {}

            EepromState::Receiving => {
                // Ignore leading zeros — wait for the start bit (first 1) before counting.
                if self.in_count == 0 && !di {
                    return;
                }

                self.in_bits   = (self.in_bits << 1) | (di as u32);
                self.in_count += 1;

                if self.in_count < 10 {
                    return;
                }

                // The first bit counted is always 1 (start bit), so no extra check needed.

                let op   = ((self.in_bits >> 7) & 0x3) as u8;
                let addr = (self.in_bits & 0x7F) as u8;
                self.in_bits  = 0;
                self.in_count = 0;

                match op {
                    0b10 => {
                        // READ — shift out 16-bit word (dummy 0 bit, then MSB first)
                        let word = self.read_word(addr & 0x7F);
                        self.state   = EepromState::Reading { word, sent: 0 };
                        self.do_bit  = false; // dummy zero before data
                    }
                    0b01 => {
                        // WRITE — collect 16 more bits, then store
                        self.state = EepromState::Writing { addr: addr & 0x7F, data: 0, received: 0 };
                    }
                    0b11 => {
                        // ERASE — clear single word
                        if self.write_enabled {
                            self.write_word(addr & 0x7F, 0xFFFF);
                        }
                        self.do_bit = true; // write-complete indicator
                        self.state  = EepromState::Idle;
                    }
                    0b00 => {
                        // Special commands decoded by upper 2 bits of address
                        match (addr >> 5) & 0x3 {
                            0b11 => self.write_enabled = true,   // WREN
                            0b00 => self.write_enabled = false,  // EWDS
                            0b10 => {
                                // ERAL — erase all words
                                if self.write_enabled {
                                    self.data.fill(0xFF);
                                }
                            }
                            _ => {
                                // WRAL — write all: collect 16-bit data
                                self.state = EepromState::WritingAll { data: 0, received: 0 };
                                return;
                            }
                        }
                        self.do_bit = true;
                        self.state  = EepromState::Idle;
                    }
                    _ => unreachable!(),
                }
            }

            EepromState::Reading { word, sent } => {
                // Dummy bit already output on entry; shift data MSB-first.
                if sent < 16 {
                    self.do_bit = (word >> (15 - sent)) & 1 != 0;
                    self.state  = EepromState::Reading { word, sent: sent + 1 };
                } else {
                    self.do_bit = true;
                    self.state  = EepromState::Idle;
                }
            }

            EepromState::Writing { addr, data, received } => {
                let data = (data << 1) | (di as u16);
                if received + 1 < 16 {
                    self.state = EepromState::Writing { addr, data, received: received + 1 };
                } else {
                    if self.write_enabled {
                        self.write_word(addr, data);
                    }
                    self.do_bit = true;
                    self.state  = EepromState::Idle;
                }
            }

            EepromState::WritingAll { data, received } => {
                let data = (data << 1) | (di as u16);
                if received + 1 < 16 {
                    self.state = EepromState::WritingAll { data, received: received + 1 };
                } else {
                    if self.write_enabled {
                        for i in 0..128 {
                            self.write_word(i, data);
                        }
                    }
                    self.do_bit = true;
                    self.state  = EepromState::Idle;
                }
            }
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn read_word(&self, addr: u8) -> u16 {
        let i = (addr as usize) * 2;
        u16::from_le_bytes([self.data[i], self.data[i + 1]])
    }

    fn write_word(&mut self, addr: u8, val: u16) {
        let i = (addr as usize) * 2;
        let bytes = val.to_le_bytes();
        self.data[i]     = bytes[0];
        self.data[i + 1] = bytes[1];
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn load_bytes(&mut self, bytes: &[u8]) {
        let len = bytes.len().min(256);
        self.data[..len].copy_from_slice(&bytes[..len]);
    }
}

// ── MBC7 ─────────────────────────────────────────────────────────────────────

pub struct Mbc7 {
    rom: Vec<u8>,
    rom_bank: u16,

    /// Gate 1: write 0x0A to 0x0000-0x1FFF.
    ram_gate1: bool,
    /// Gate 2: write 0x40 to 0x4000-0x5FFF. Both gates must be open for RAM access.
    ram_gate2: bool,

    // Accelerometer (ADXL202E)
    accel_x: u16,          // current host value; center ≈ 0x81D0
    accel_y: u16,
    accel_x_latched: u16,  // snapshot taken on the 0x55/0xAA write sequence
    accel_y_latched: u16,
    latch_step: LatchStep,

    eeprom: Eeprom93lc56,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LatchStep {
    Idle,
    Seen55, // received 0x55, waiting for 0xAA
}

impl Mbc7 {
    pub fn new(rom: Vec<u8>) -> Self {
        Mbc7 {
            rom,
            rom_bank: 1,
            ram_gate1: false,
            ram_gate2: false,
            accel_x: ACCEL_CENTER as u16,
            accel_y: ACCEL_CENTER as u16,
            accel_x_latched: ACCEL_CENTER as u16,
            accel_y_latched: ACCEL_CENTER as u16,
            latch_step: LatchStep::Idle,
            eeprom: Eeprom93lc56::new(),
        }
    }

    /// Feed a new accelerometer reading from the host (WASM or iOS).
    ///
    /// `x` and `y` are signed offsets from flat/center in host units where ±0x1000 = ±1g.
    ///
    /// The ADXL202E on the MBC7 hardware maps ±1g to ±0x70 (±112 counts) around the center
    /// value of 0x81D0. We scale from the host's ±0x1000-per-g to the hardware ±0x70-per-g here.
    pub fn set_accelerometer(&mut self, x: i32, y: i32) {
        // Scale: host ±0x1000 per g → hardware ±0x70 per g
        let sx = (x * 0x70) / 0x1000;
        let sy = (y * 0x70) / 0x1000;
        self.accel_x = (ACCEL_CENTER + sx).clamp(0, 0xFFFF) as u16;
        self.accel_y = (ACCEL_CENTER + sy).clamp(0, 0xFFFF) as u16;
    }

    fn ram_open(&self) -> bool {
        self.ram_gate1 && self.ram_gate2
    }
}

impl Cartridge for Mbc7 {
    fn read_rom(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF),
            0x4000..=0x7FFF => {
                let bank   = self.rom_bank as usize;
                let offset = bank * ROM_BANK_SIZE + (addr as usize - 0x4000);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write_rom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_gate1 = value == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = value as u16,
            0x4000..=0x5FFF => self.ram_gate2 = value == 0x40,
            _ => {}
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_open() {
            return 0xFF;
        }
        // Address bits 4-7 select the register; bits 0-3 and 8-11 are ignored.
        let reg = (addr >> 4) & 0x0F;
        match reg {
            0x0 => 0xFF, // latch step 1 — write-only
            0x1 => 0xFF, // latch step 2 — write-only
            0x2 => (self.accel_x_latched & 0xFF) as u8,
            0x3 => (self.accel_x_latched >> 8) as u8,
            0x4 => (self.accel_y_latched & 0xFF) as u8,
            0x5 => (self.accel_y_latched >> 8) as u8,
            0x6 => 0x00, // Z-axis LSB (always 0)
            0x7 => 0xFF, // Z-axis MSB (always 0xFF)
            _   => self.eeprom.read(), // reg 8-15 → EEPROM
        }
    }

    fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_open() {
            return;
        }
        // Address bits 4-7 select the register; bits 0-3 and 8-11 are ignored.
        let reg = (addr >> 4) & 0x0F;
        match reg {
            0x0 => {
                // Latch step 1: 0x55 arms the latch; latched data unchanged until 0xAA
                if value == 0x55 {
                    self.latch_step = LatchStep::Seen55;
                } else {
                    self.latch_step = LatchStep::Idle;
                }
            }
            0x1 => {
                // Latch step 2: 0xAA captures current accelerometer reading
                if self.latch_step == LatchStep::Seen55 && value == 0xAA {
                    self.accel_x_latched = self.accel_x;
                    self.accel_y_latched = self.accel_y;
                }
                self.latch_step = LatchStep::Idle;
            }
            0x8..=0xF => self.eeprom.write(value),
            _ => {} // regs 2-7 are read-only sensor registers; writes ignored
        }
    }

    fn ram_data(&self) -> &[u8] {
        self.eeprom.as_bytes()
    }

    fn load_ram(&mut self, data: &[u8]) {
        self.eeprom.load_bytes(data);
    }

    fn mbc_type(&self) -> MbcType {
        MbcType::Mbc7
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len() / ROM_BANK_SIZE
    }

    fn current_rom_bank(&self) -> u16 {
        self.rom_bank
    }

    fn is_ram_enabled(&self) -> bool {
        self.ram_open()
    }

    fn as_mbc7_mut(&mut self) -> Option<&mut Mbc7> {
        Some(self)
    }
}
