//! Kernel logging subsystem.
#![no_std]

use core::fmt;

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => " INFO",
            Level::Warn => " WARN",
            Level::Error => "ERROR",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Level::Trace => "\x1b[90m", // Gray
            Level::Debug => "\x1b[36m", // Cyan
            Level::Info => "\x1b[32m",  // Green
            Level::Warn => "\x1b[33m",  // Yellow
            Level::Error => "\x1b[31m", // Red
        }
    }
}

/// Initialize the kernel logger (sets up serial port)
pub fn init() {
    khal::serial::init();
}

/// Log a message with a specific level
pub fn log(level: Level, args: fmt::Arguments) {
    khal::serial::write_str(level.color());
    khal::serial::write_str("[");
    khal::serial::write_str(level.as_str());
    khal::serial::write_str("]\x1b[0m ");
    khal::serial::write_fmt(args);
    khal::serial::write_str("\n");
}

/// Print to serial without formatting
pub fn print(args: fmt::Arguments) {
    khal::serial::write_fmt(args);
}

/// Log at TRACE level
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Trace, format_args!($($arg)*))
    };
}

/// Log at DEBUG level
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Debug, format_args!($($arg)*))
    };
}

/// Log at INFO level
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Info, format_args!($($arg)*))
    };
}

/// Log at WARN level
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Warn, format_args!($($arg)*))
    };
}

/// Log at ERROR level
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Error, format_args!($($arg)*))
    };
}

/// Print without newline
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print(format_args!($($arg)*))
    };
}

/// Print with newline
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        $crate::print(format_args!($($arg)*));
        $crate::print(format_args!("\n"));
    }};
}
