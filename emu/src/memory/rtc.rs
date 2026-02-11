//! MBC3 Real-Time Clock (RTC) emulation.
//!
//! The RTC provides seconds, minutes, hours, and a 9-bit day counter
//! accessible through RAM bank registers 0x08-0x0C. A latch mechanism
//! (write 0x00 then 0x01 to 0x6000-0x7FFF) freezes a snapshot for
//! consistent reads.

#[cfg(target_arch = "wasm32")]
fn now_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) struct Rtc {
    // Live registers
    s: u8,
    m: u8,
    h: u8,
    dl: u8,
    dh: u8,

    // Latched copies (frozen snapshot for game reads)
    latched_s: u8,
    latched_m: u8,
    latched_h: u8,
    latched_dl: u8,
    latched_dh: u8,

    // Latch detector: write 0x00 then 0x01 triggers latch
    latch_ready: bool,

    // Unix timestamp (seconds) when live registers were last synced
    base_timestamp: u64,
}

impl Rtc {
    pub fn new() -> Self {
        Rtc {
            s: 0,
            m: 0,
            h: 0,
            dl: 0,
            dh: 0,
            latched_s: 0,
            latched_m: 0,
            latched_h: 0,
            latched_dl: 0,
            latched_dh: 0,
            latch_ready: false,
            base_timestamp: now_secs(),
        }
    }

    /// Advance live registers based on wall-clock elapsed time.
    pub fn tick(&mut self) {
        // Halted — don't advance
        if self.dh & 0x40 != 0 {
            self.base_timestamp = now_secs();
            return;
        }

        let now = now_secs();
        let elapsed = now.saturating_sub(self.base_timestamp);
        if elapsed == 0 {
            return;
        }
        self.base_timestamp = now;

        // Convert current registers to total seconds
        let day = ((self.dh as u32 & 0x01) << 8) | self.dl as u32;
        let mut total_secs =
            day as u64 * 86400 + self.h as u64 * 3600 + self.m as u64 * 60 + self.s as u64;

        total_secs += elapsed;

        self.s = (total_secs % 60) as u8;
        total_secs /= 60;
        self.m = (total_secs % 60) as u8;
        total_secs /= 60;
        self.h = (total_secs % 24) as u8;
        total_secs /= 24;

        let days = total_secs as u32 + day;
        if days > 511 {
            // Day counter overflow — set carry, wrap to 0
            self.dh = (self.dh & 0x40) | 0x80; // preserve halt, set carry, clear day MSB
            self.dl = 0;
        } else {
            self.dl = days as u8;
            self.dh = (self.dh & 0xC0) | ((days >> 8) & 0x01) as u8;
        }
    }

    /// Handle writes to 0x6000-0x7FFF for latch. Write 0x00 then 0x01 to latch.
    pub fn write_latch(&mut self, value: u8) {
        if value == 0x00 {
            self.latch_ready = true;
        } else if value == 0x01 && self.latch_ready {
            self.latch_ready = false;
            self.latched_s = self.s;
            self.latched_m = self.m;
            self.latched_h = self.h;
            self.latched_dl = self.dl;
            self.latched_dh = self.dh;
        } else {
            self.latch_ready = false;
        }
    }

    /// Read a latched RTC register. `reg` is the RAM bank value (0x08-0x0C).
    pub fn read_register(&self, reg: u8) -> u8 {
        match reg {
            0x08 => self.latched_s,
            0x09 => self.latched_m,
            0x0A => self.latched_h,
            0x0B => self.latched_dl,
            0x0C => self.latched_dh,
            _ => 0xFF,
        }
    }

    /// Write a live RTC register. `reg` is the RAM bank value (0x08-0x0C).
    /// Resets the base timestamp so elapsed-time tracking restarts from the
    /// newly written values.
    pub fn write_register(&mut self, reg: u8, value: u8) {
        match reg {
            0x08 => self.s = value & 0x3F,
            0x09 => self.m = value & 0x3F,
            0x0A => self.h = value & 0x1F,
            0x0B => self.dl = value,
            0x0C => self.dh = value,
            _ => {}
        }
        self.base_timestamp = now_secs();
    }

    /// Returns true when the selected bank maps to an RTC register.
    pub fn is_rtc_register(bank: u8) -> bool {
        (0x08..=0x0C).contains(&bank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_zeroed() {
        let rtc = Rtc::new();
        assert_eq!(rtc.s, 0);
        assert_eq!(rtc.m, 0);
        assert_eq!(rtc.h, 0);
        assert_eq!(rtc.dl, 0);
        assert_eq!(rtc.dh, 0);
    }

    #[test]
    fn test_is_rtc_register() {
        assert!(!Rtc::is_rtc_register(0x07));
        assert!(Rtc::is_rtc_register(0x08));
        assert!(Rtc::is_rtc_register(0x0C));
        assert!(!Rtc::is_rtc_register(0x0D));
    }

    #[test]
    fn test_write_and_read_registers() {
        let mut rtc = Rtc::new();

        // Write live registers
        rtc.write_register(0x08, 30); // seconds
        rtc.write_register(0x09, 45); // minutes
        rtc.write_register(0x0A, 12); // hours
        rtc.write_register(0x0B, 0xFF); // day low (full byte)
        rtc.write_register(0x0C, 0x01); // day high bit 0

        // Latched values should still be 0 until we latch
        assert_eq!(rtc.read_register(0x08), 0);

        // Latch
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);

        assert_eq!(rtc.read_register(0x08), 30);
        assert_eq!(rtc.read_register(0x09), 45);
        assert_eq!(rtc.read_register(0x0A), 12);
        assert_eq!(rtc.read_register(0x0B), 0xFF);
        assert_eq!(rtc.read_register(0x0C), 0x01);
    }

    #[test]
    fn test_seconds_mask() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 0xFF);
        // Should be masked to 6 bits (0x3F = 63)
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 0x3F);
    }

    #[test]
    fn test_hours_mask() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x0A, 0xFF);
        // Should be masked to 5 bits (0x1F = 31)
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x0A), 0x1F);
    }

    #[test]
    fn test_latch_requires_sequence() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 42);

        // Just writing 0x01 without prior 0x00 should not latch
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 0);

        // Writing 0x00, then something else, then 0x01 should not latch
        rtc.write_latch(0x00);
        rtc.write_latch(0x02);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 0);

        // Correct sequence
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 42);
    }

    #[test]
    fn test_latch_freezes_snapshot() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 10);

        // Latch
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 10);

        // Change live register — latched should stay frozen
        rtc.write_register(0x08, 20);
        assert_eq!(rtc.read_register(0x08), 10);

        // Re-latch
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 20);
    }

    #[test]
    fn test_tick_advances_time() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 58); // 58 seconds

        // Simulate 5 seconds passing by rewinding base_timestamp
        rtc.base_timestamp = now_secs() - 5;
        rtc.tick();

        // 58 + 5 = 63 seconds → 1 minute, 3 seconds
        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 3);
        assert_eq!(rtc.read_register(0x09), 1);
    }

    #[test]
    fn test_tick_rollover_minutes_hours() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 0);
        rtc.write_register(0x09, 59);
        rtc.write_register(0x0A, 23);
        rtc.write_register(0x0B, 0);

        // Simulate 1 hour passing (3600 seconds)
        rtc.base_timestamp = now_secs() - 3600;
        rtc.tick();

        rtc.write_latch(0x00);
        rtc.write_latch(0x01);

        // 23:59:00 + 1:00:00 = day 1, 00:59:00
        assert_eq!(rtc.read_register(0x08), 0);
        assert_eq!(rtc.read_register(0x09), 59);
        assert_eq!(rtc.read_register(0x0A), 0);
        assert_eq!(rtc.read_register(0x0B), 1);
    }

    #[test]
    fn test_halt_prevents_advance() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 10);
        rtc.write_register(0x0C, 0x40); // set halt bit

        // Simulate time passing
        rtc.base_timestamp = now_secs() - 100;
        rtc.tick();

        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 10); // unchanged
    }

    #[test]
    fn test_day_counter_carry() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x0B, 0xFF); // day low = 255
        rtc.write_register(0x0C, 0x01); // day high bit = 1, total = 511

        // Advance by 1 day
        rtc.base_timestamp = now_secs() - 86400;
        rtc.tick();

        rtc.write_latch(0x00);
        rtc.write_latch(0x01);

        // Day 512 overflows → carry set, days wrap to 0
        assert_eq!(rtc.read_register(0x0B), 0); // day low
        assert_eq!(rtc.read_register(0x0C) & 0x80, 0x80); // carry flag set
        assert_eq!(rtc.read_register(0x0C) & 0x01, 0x00); // day MSB cleared
    }

    #[test]
    fn test_read_invalid_register() {
        let rtc = Rtc::new();
        assert_eq!(rtc.read_register(0x00), 0xFF);
        assert_eq!(rtc.read_register(0x0D), 0xFF);
    }

    #[test]
    fn test_zero_elapsed_no_change() {
        let mut rtc = Rtc::new();
        rtc.write_register(0x08, 30);
        // base_timestamp is now, so elapsed = 0
        rtc.tick();

        rtc.write_latch(0x00);
        rtc.write_latch(0x01);
        assert_eq!(rtc.read_register(0x08), 30);
    }
}
