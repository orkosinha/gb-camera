//! Game Boy Color (GBC) state and palette management.
//!
//! Holds the GBC-specific hardware state that is orthogonal to the main
//! memory/banking machinery: colour palette RAM (64 bytes BG + 64 bytes OBJ),
//! the speed-switch flag, and the HDMA tracking registers.
//!
//! The VRAM and WRAM banking arrays live in `Memory` because they are also
//! accessed by the PPU and general bus; only the *control* state (bank index,
//! armed flag, etc.) sits here.

/// All Game Boy Color–specific emulator state.
pub struct Cgb {
    /// GBC mode active (set explicitly by the caller, never auto-detected).
    pub mode: bool,

    /// BG colour palette RAM — 8 palettes × 4 colours × 2 bytes (RGB555 LE).
    pub bg_palette_ram: [u8; 64],
    /// OBJ colour palette RAM — same layout.
    pub obj_palette_ram: [u8; 64],
    /// FF68: BG palette index register (bit 7 = auto-increment, bits 5-0 = address).
    pub bcps: u8,
    /// FF6A: OBJ palette index register (same layout as bcps).
    pub ocps: u8,

    /// Current VRAM bank (0 or 1); controlled by VBK (0xFF4F).
    pub vram_bank: usize,
    /// Current switchable WRAM bank (1-7); writing 0 to SVBK also maps bank 1.
    pub wram_bank: usize,

    /// Double-speed CPU mode is currently active.
    pub double_speed: bool,
    /// KEY1 bit 0 – speed switch has been requested (will fire on next STOP).
    pub speed_armed: bool,

    /// HDMA source address (assembled from HDMA1/2; lower 4 bits masked to 0).
    pub hdma_source: u16,
    /// HDMA destination address within VRAM (lower 4 bits masked; bit 15 = 1).
    pub hdma_dest: u16,
    /// Remaining 16-byte blocks for the active H-blank DMA.
    pub hdma_len: u8,
    /// An HDMA transfer is currently in progress.
    pub hdma_active: bool,
    /// true = H-blank mode DMA; false = general-purpose (one-shot) DMA.
    pub hdma_hblank: bool,
}

impl Cgb {
    pub fn new() -> Self {
        Cgb {
            mode: false,
            bg_palette_ram: [0; 64],
            obj_palette_ram: [0; 64],
            bcps: 0,
            ocps: 0,
            vram_bank: 0,
            wram_bank: 1,
            double_speed: false,
            speed_armed: false,
            hdma_source: 0,
            hdma_dest: 0,
            hdma_len: 0,
            hdma_active: false,
            hdma_hblank: false,
        }
    }

    /// Read two bytes (lo, hi) from the BG palette for a given palette and colour index.
    #[inline]
    pub fn read_bg_palette(&self, palette: usize, color: usize) -> (u8, u8) {
        let offset = palette * 8 + color * 2;
        (self.bg_palette_ram[offset], self.bg_palette_ram[offset + 1])
    }

    /// Read two bytes (lo, hi) from the OBJ palette for a given palette and colour index.
    #[inline]
    pub fn read_obj_palette(&self, palette: usize, color: usize) -> (u8, u8) {
        let offset = palette * 8 + color * 2;
        (self.obj_palette_ram[offset], self.obj_palette_ram[offset + 1])
    }

    /// Toggle double-speed mode (invoked by the STOP opcode when KEY1 bit 0 is set).
    #[inline]
    pub fn toggle_double_speed(&mut self) {
        self.double_speed = !self.double_speed;
        self.speed_armed = false;
    }
}

impl Default for Cgb {
    fn default() -> Self {
        Self::new()
    }
}
