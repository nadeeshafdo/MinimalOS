//! Kernel keyboard input buffer — [068].
//!
//! A fixed-size ring buffer that sits between the IRQ1 handler
//! (producer) and `sys_read(STDIN)` (consumer).

use spin::Mutex;

const BUF_SIZE: usize = 256;

/// A simple ring buffer for keyboard characters.
struct RingBuffer {
    buf: [u8; BUF_SIZE],
    read: usize,
    write: usize,
    count: usize,
}

impl RingBuffer {
    const fn new() -> Self {
        Self {
            buf: [0; BUF_SIZE],
            read: 0,
            write: 0,
            count: 0,
        }
    }

    /// Push a byte into the buffer.  Drops silently if full.
    fn push(&mut self, byte: u8) {
        if self.count >= BUF_SIZE {
            return; // full — drop
        }
        self.buf[self.write] = byte;
        self.write = (self.write + 1) % BUF_SIZE;
        self.count += 1;
    }

    /// Pop a byte from the buffer.  Returns `None` if empty.
    fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            return None;
        }
        let byte = self.buf[self.read];
        self.read = (self.read + 1) % BUF_SIZE;
        self.count -= 1;
        Some(byte)
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }
}

static INPUT: Mutex<RingBuffer> = Mutex::new(RingBuffer::new());

/// Called by the keyboard IRQ handler to enqueue a character.
pub fn push_char(ch: char) {
    // Only buffer printable ASCII + control chars (newline, backspace).
    if ch.is_ascii() {
        INPUT.lock().push(ch as u8);
    }
}

/// Called by `sys_read(STDIN)` to dequeue a character.
///
/// Returns 0 if the buffer is empty (non-blocking).
pub fn pop_char() -> u8 {
    INPUT.lock().pop().unwrap_or(0)
}

/// Returns true if there is at least one character available.
pub fn has_input() -> bool {
    !INPUT.lock().is_empty()
}
