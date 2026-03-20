//! Lightweight debug logging helpers (no-ops in release).

use std::fmt::Arguments;
use std::sync::OnceLock;
use std::thread;
use std::time::Instant;

pub const RESET:  &str = "\x1b[0m";
pub const DIM:    &str = "\x1b[2m";
pub const BOLD:   &str = "\x1b[1m";
pub const RED:    &str = "\x1b[31m";
pub const GREEN:  &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN:   &str = "\x1b[36m";
pub const GRAY:   &str = "\x1b[90m";

static DEMO_START: OnceLock<Instant> = OnceLock::new();

/// Call once at the start of `run_demo` to anchor relative timestamps.
pub fn init_demo_start() {
    DEMO_START.get_or_init(Instant::now);
}

/// Print a debug log line when compiled with debug assertions.
pub fn dev_log(args: Arguments) {
    if !cfg!(debug_assertions) { return; }
    let ts = DEMO_START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0);
    let thread_name = thread::current();
    let thread_name = thread_name.name().unwrap_or("unnamed");
    let msg = format!("{args}");
    // Pad the leading [TAG] to 8 chars so message content starts at a fixed column.
    let msg = if let Some(end) = msg.find(']') {
        let tag = &msg[..=end];
        let rest = &msg[end + 1..];
        format!("{tag:<8}{rest}")
    } else {
        msg
    };
    let color = if msg.contains("[QUEUE]") {
        CYAN
    } else if msg.contains("[ZONE]") {
        YELLOW
    } else if msg.contains("offline") || msg.contains("stops heartbeats") {
        RED
    } else if msg.contains("[HEALTH]") {
        GREEN
    } else {
        GRAY
    };
    println!("{DIM}[+{ts:>5}ms][{thread_name:^14}]{RESET} {color}{msg}{RESET}");
}

/// Convenience macro for debug-only logging.
#[macro_export]
macro_rules! log_dev {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            $crate::logging::dev_log(format_args!($($arg)*));
        }
    };
}
