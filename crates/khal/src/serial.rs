//! Serial port (COM1 UART 16550) driver.

use core::fmt;
use spin::Mutex;

use crate::port::{inb, outb};

/// COM1 base port address
const COM1_PORT: u16 = 0x3F8;

/// Serial port driver for COM1
pub struct Serial {
    initialized: bool,
}

impl Serial {
    /// Create a new uninitialized Serial port instance
    const fn new() -> Self {
        Self {
            initialized: false,
        }
    }

    /// Initialize the serial port (115200 baud, 8N1)
    pub fn init(&mut self) {
        unsafe {
            // Disable all interrupts
            outb(COM1_PORT + 1, 0x00);

            // Enable DLAB (set baud rate divisor)
            outb(COM1_PORT + 3, 0x80);

            // Set divisor to 1 (115200 baud)
            outb(COM1_PORT + 0, 0x01); // Divisor low byte
            outb(COM1_PORT + 1, 0x00); // Divisor high byte

            // 8 bits, no parity, one stop bit (clear DLAB)
            outb(COM1_PORT + 3, 0x03);

            // Enable FIFO, clear them, with 14-byte threshold
            outb(COM1_PORT + 2, 0xC7);

            // Set RTS/DSR, disable IRQ gate (OUT2=0)
            outb(COM1_PORT + 4, 0x03);

            // Put chip in loopback mode to test
            outb(COM1_PORT + 4, 0x1E);

            // Send test byte
            outb(COM1_PORT + 0, 0xAE);

            // Check if we receive same byte back
            if inb(COM1_PORT + 0) != 0xAE {
                // Serial port is faulty, but continue anyway
                self.initialized = true;
                return;
            }

            // Loopback passed - set normal operation (OUT1, OUT2, RTS, DTR)
            // OUT2 must be set for interrupts but we keep interrupts disabled
            outb(COM1_PORT + 4, 0x0F);

            // Keep interrupts disabled - we poll instead
            outb(COM1_PORT + 1, 0x00);

            self.initialized = true;
        }
    }

    /// Check if transmit buffer is empty
    fn is_transmit_empty() -> bool {
        unsafe { inb(COM1_PORT + 5) & 0x20 != 0 }
    }

    /// Write a byte to the serial port
    pub fn write_byte(&self, byte: u8) {
        if !self.initialized {
            return;
        }

        // Wait for transmit buffer to be empty
        while !Self::is_transmit_empty() {
            core::hint::spin_loop();
        }

        unsafe {
            outb(COM1_PORT, byte);
        }
    }

    /// Write a string to the serial port
    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }
}

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

/// Global serial port instance (COM1)
static SERIAL: Mutex<Serial> = Mutex::new(Serial::new());

/// Initialize the global serial port
pub fn init() {
    SERIAL.lock().init();
    // Send a test message to verify serial is working
    SERIAL.lock().write_str("Serial port initialized\n");
}

/// Write a string to the serial port
pub fn write_str(s: &str) {
    SERIAL.lock().write_str(s);
}

/// Write formatted arguments to the serial port
pub fn write_fmt(args: fmt::Arguments) {
    use fmt::Write;
    SERIAL.lock().write_fmt(args).unwrap();
}
