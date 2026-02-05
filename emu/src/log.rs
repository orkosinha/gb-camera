//! Logging infrastructure for the Game Boy emulator.
//!
//! Provides rate-limited, categorized logging for debugging without overwhelming output.

use std::sync::atomic::{AtomicU32, Ordering};

/// Log categories for filtering and rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LogCategory {
    Camera,
    Memory,
    Cpu,
    Ppu,
    General,
}

impl LogCategory {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn prefix(self) -> &'static str {
        match self {
            LogCategory::Camera => "[Camera]",
            LogCategory::Memory => "[Memory]",
            LogCategory::Cpu => "[CPU]",
            LogCategory::Ppu => "[PPU]",
            LogCategory::General => "[EMU]",
        }
    }
}

/// Rate limiter that tracks how many times a particular log point has been hit.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub struct RateLimiter {
    counter: AtomicU32,
    limit: u32,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl RateLimiter {
    /// Create a new rate limiter that allows `limit` messages.
    pub const fn new(limit: u32) -> Self {
        RateLimiter {
            counter: AtomicU32::new(0),
            limit,
        }
    }

    /// Check if we should log. Returns true if under the limit.
    pub fn should_log(&self) -> bool {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        count < self.limit
    }

    /// Check if we should log at this interval (e.g., every N calls).
    #[allow(dead_code)]
    pub fn should_log_interval(&self, interval: u32) -> bool {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        count < self.limit || count.is_multiple_of(interval)
    }

    /// Get current count without incrementing.
    #[allow(dead_code)]
    pub fn count(&self) -> u32 {
        self.counter.load(Ordering::Relaxed)
    }
}

/// Logger that outputs to the browser console.
pub struct Logger;

impl Logger {
    /// Log an info message.
    #[cfg(target_arch = "wasm32")]
    pub fn info(category: LogCategory, msg: &str) {
        let formatted = format!("{} {}", category.prefix(), msg);
        web_sys::console::log_1(&formatted.into());
    }

    /// Log a warning message.
    #[cfg(target_arch = "wasm32")]
    pub fn warn(category: LogCategory, msg: &str) {
        let formatted = format!("{} {}", category.prefix(), msg);
        web_sys::console::warn_1(&formatted.into());
    }

    /// Log an error message.
    #[cfg(target_arch = "wasm32")]
    #[allow(dead_code)]
    pub fn error(category: LogCategory, msg: &str) {
        let formatted = format!("{} {}", category.prefix(), msg);
        web_sys::console::error_1(&formatted.into());
    }

    /// Log with rate limiting.
    #[cfg(target_arch = "wasm32")]
    pub fn info_limited(category: LogCategory, limiter: &RateLimiter, msg: &str) {
        if limiter.should_log() {
            Self::info(category, msg);
        }
    }

    /// Log with rate limiting at intervals.
    #[cfg(target_arch = "wasm32")]
    #[allow(dead_code)]
    pub fn info_interval(category: LogCategory, limiter: &RateLimiter, interval: u32, msg: &str) {
        if limiter.should_log_interval(interval) {
            Self::info(category, msg);
        }
    }

    // No-op implementations for non-WASM builds
    #[cfg(not(target_arch = "wasm32"))]
    pub fn info(_category: LogCategory, _msg: &str) {}

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn warn(_category: LogCategory, _msg: &str) {}

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn error(_category: LogCategory, _msg: &str) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn info_limited(_category: LogCategory, _limiter: &RateLimiter, _msg: &str) {}

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn info_interval(
        _category: LogCategory,
        _limiter: &RateLimiter,
        _interval: u32,
        _msg: &str,
    ) {
    }
}

/// Convenience macros for logging.
#[macro_export]
macro_rules! log_info {
    ($cat:expr, $($arg:tt)*) => {
        $crate::log::Logger::info($cat, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($cat:expr, $limiter:expr, $($arg:tt)*) => {
        if $limiter.should_log() {
            $crate::log::Logger::warn($cat, &format!($($arg)*))
        }
    };
    ($cat:expr, $($arg:tt)*) => {
        $crate::log::Logger::warn($cat, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info_limited {
    ($cat:expr, $limiter:expr, $($arg:tt)*) => {
        $crate::log::Logger::info_limited($cat, $limiter, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info_interval {
    ($cat:expr, $limiter:expr, $interval:expr, $($arg:tt)*) => {
        $crate::log::Logger::info_interval($cat, $limiter, $interval, &format!($($arg)*))
    };
}
