// =============================================================================
// MinimalOS NextGen — Serial UART Driver (COM1)
// =============================================================================
//
// This is the simplest and most reliable output device on x86.
// It outputs text over the serial port, which is captured by:
//   - QEMU's `-serial stdio` flag (shows in the terminal)
//   - A physical serial cable (if you had one on your HP Notebook)
//   - QEMU's debug console in general
//
// WHY SERIAL FIRST?
//   Serial is the FIRST thing we bring up because:
//   1. It requires NO memory allocation (just I/O port writes)
//   2. It requires NO interrupts (we poll the status register)
//   3. It requires NO page tables (I/O ports are in a separate address space)
//   4. It works before ANYTHING else is initialized
//   5. If something breaks, we can still see debug output up to the crash
//
// HARDWARE DETAILS:
//   The 16550 UART is the standard serial chip on x86 PCs.
//   COM1 lives at I/O port base 0x3F8 with 8 registers:
//
//   Port    │ Read              │ Write
//   ────────┼───────────────────┼──────────────────
//   +0      │ Receive Buffer    │ Transmit Holding
//   +1      │ Interrupt Enable  │ Interrupt Enable
//   +2      │ Interrupt ID      │ FIFO Control
//   +3      │ Line Control      │ Line Control
//   +4      │ Modem Control     │ Modem Control
//   +5      │ Line Status       │ (factory test)
//   +6      │ Modem Status      │ (not used)
//   +7      │ Scratch           │ Scratch
//
//   When DLAB (Divisor Latch Access Bit) is set in Line Control:
//   +0      │ Divisor Latch Low │ Divisor Latch Low
//   +1      │ Divisor Latch High│ Divisor Latch High
//
//   We configure: 115200 baud, 8 data bits, no parity, 1 stop bit (8N1).
//   This is the standard configuration used by QEMU and most serial tools.
//
// BAUD RATE CALCULATION:
//   The UART's base clock is 1.8432 MHz (115200 × 16).
//   Divisor = 115200 × 16 / desired_baud = 1843200 / 115200 = 16... wait.
//   Actually: divisor = 115200 / desired_baud.
//   For 115200 baud: divisor = 1 (this is the maximum speed).
//
// THREAD SAFETY:
//   The serial port is a global resource accessed from multiple cores
//   and from interrupt handlers. We protect it with our SpinLock.
//   The lock ensures characters from different kprintln!() calls don't
//   get interleaved.
//
// =============================================================================

use crate::sync::spinlock::SpinLock;
use core::fmt;
use core::fmt::Write;

// =============================================================================
// I/O Port Addresses for COM1
// =============================================================================

/// Base I/O port for COM1. This is standardized on all x86 PCs.
const COM1_BASE: u16 = 0x3F8;

/// Register offsets from the base port.
/// These map to the 16550 UART register layout.
const DATA_REG: u16 = 0;         // +0: TX/RX data (or divisor low when DLAB=1)
const INT_ENABLE_REG: u16 = 1;   // +1: Interrupt enable (or divisor high when DLAB=1)
const FIFO_CTRL_REG: u16 = 2;    // +2: FIFO control
const LINE_CTRL_REG: u16 = 3;    // +3: Line control (data bits, parity, stop bits)
const MODEM_CTRL_REG: u16 = 4;   // +4: Modem control (DTR, RTS, loopback)
const LINE_STATUS_REG: u16 = 5;  // +5: Line status (TX empty, RX ready, errors)

/// Line Status Register bit masks
const LSR_TX_EMPTY: u8 = 1 << 5;   // Transmit Holding Register Empty
const LSR_RX_READY: u8 = 1 << 0;   // Data Ready (byte received)

// =============================================================================
// Global Serial Port Instance
// =============================================================================

/// The global serial port, protected by a spinlock.
///
/// This is the kernel's primary debug output. It's initialized in Phase 1
/// of boot before anything else. The SpinLock ensures that multi-core
/// output doesn't interleave.
///
/// Usage (via the kprintln! macro, not directly):
///   kprintln!("Hello from core {}", cpu_id);
pub static SERIAL: SpinLock<SerialPort> = SpinLock::new(SerialPort::new(COM1_BASE));

// =============================================================================
// SerialPort Implementation
// =============================================================================

/// Represents a 16550 UART serial port.
///
/// This struct is `!Send` by intent — the global instance is always accessed
/// through the `SERIAL` spinlock. Direct construction is allowed for testing
/// in single-threaded contexts.
pub struct SerialPort {
    /// Base I/O port address for this UART.
    base: u16,
}

impl SerialPort {
    /// Creates a new SerialPort at the given base I/O port.
    ///
    /// This doesn't touch hardware — call `init()` to configure the UART.
    /// Using `const fn` so it can be used in static initialization.
    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    /// Initializes the UART hardware.
    ///
    /// Must be called once during boot before any output.
    /// Configures the UART for 115200 baud, 8N1.
    ///
    /// The initialization sequence follows the 16550 UART programming guide:
    ///   1. Disable all interrupts (we poll, not interrupt-driven)
    ///   2. Set baud rate divisor (DLAB must be set first)
    ///   3. Configure data format (8 data bits, no parity, 1 stop bit)
    ///   4. Enable and clear FIFOs (16-byte hardware buffer)
    ///   5. Set modem control (DTR + RTS + OUT2 to enable interrupts later)
    ///   6. Test with loopback mode to verify hardware works
    pub fn init(&self) {
        // Step 1: Disable all UART interrupts.
        // We use polling mode during early boot. Interrupt-driven serial
        // might be added later for efficiency, but polling is simpler and
        // works without an IDT.
        self.write_port(INT_ENABLE_REG, 0x00);

        // Step 2: Set baud rate.
        // To access the divisor latch, we must set DLAB (bit 7) in
        // the Line Control Register.
        self.write_port(LINE_CTRL_REG, 0x80); // Set DLAB = 1

        // Write divisor value. For 115200 baud, divisor = 1.
        //   Divisor low byte:  1 (port +0 when DLAB=1)
        //   Divisor high byte: 0 (port +1 when DLAB=1)
        self.write_port(DATA_REG, 0x01);      // Divisor low: 1
        self.write_port(INT_ENABLE_REG, 0x00); // Divisor high: 0

        // Step 3: Configure data format.
        // Clear DLAB (bit 7 = 0) and set format:
        //   Bits 0-1 = 11: 8 data bits
        //   Bit 2    = 0:  1 stop bit
        //   Bits 3-5 = 000: no parity
        //   Bit 7    = 0:  DLAB off (back to normal register access)
        self.write_port(LINE_CTRL_REG, 0x03); // 8N1

        // Step 4: Enable and configure FIFOs.
        //   Bit 0 = 1: Enable FIFOs
        //   Bit 1 = 1: Clear receive FIFO
        //   Bit 2 = 1: Clear transmit FIFO
        //   Bits 6-7 = 11: 14-byte trigger level
        // The FIFO buffers up to 16 bytes, reducing overhead for burst writes.
        self.write_port(FIFO_CTRL_REG, 0xC7); // Enable FIFOs, clear, 14-byte trigger

        // Step 5: Set modem control.
        //   Bit 0 = 1: DTR (Data Terminal Ready)
        //   Bit 1 = 1: RTS (Request To Send)
        //   Bit 3 = 1: OUT2 (required for interrupt delivery on most UARTs)
        //   Bit 4 = 0: NOT in loopback mode (normal operation after test)
        self.write_port(MODEM_CTRL_REG, 0x0B); // DTR + RTS + OUT2

        // Step 6: Self-test with loopback mode.
        // Set loopback mode (bit 4 = 1) along with DTR, RTS, OUT1, OUT2.
        self.write_port(MODEM_CTRL_REG, 0x1E); // Loopback mode

        // Send a test byte and verify we receive it back.
        self.write_port(DATA_REG, 0xAE);

        // Check if we got it back. If not, the UART is faulty, but we
        // continue anyway — we have no other debug output this early.
        if self.read_port(DATA_REG) != 0xAE {
            // UART self-test failed. Not much we can do about it.
            // Continue and hope for the best.
            return;
        }

        // Step 7: Switch back to normal operation.
        // Set DTR + RTS + OUT2, loopback off.
        self.write_port(MODEM_CTRL_REG, 0x0F);
    }

    /// Sends a single byte over the serial port.
    ///
    /// This function busy-waits until the UART's transmit buffer has space,
    /// then writes the byte. On a 115200 baud connection, one byte takes
    /// about 87μs to transmit (10 bits per byte: start + 8 data + stop).
    ///
    /// We don't add a timeout because:
    ///   1. In QEMU, the transmit buffer is always ready immediately
    ///   2. On real hardware, 87μs is fast enough that we won't notice
    ///   3. If serial is truly stuck, we WANT to hang here — it means
    ///      something is very wrong with the hardware
    pub fn write_byte(&self, byte: u8) {
        // Wait for the Transmit Holding Register to be empty.
        // This means the UART is ready to accept another byte.
        while self.read_port(LINE_STATUS_REG) & LSR_TX_EMPTY == 0 {
            core::hint::spin_loop();
        }

        // Write the byte to the data register.
        self.write_port(DATA_REG, byte);
    }

    /// Reads a single byte from the serial port, if available.
    ///
    /// Returns `Some(byte)` if data is available, `None` if the receive
    /// buffer is empty. This is non-blocking.
    ///
    /// Useful for serial console input (shell interaction).
    pub fn read_byte(&self) -> Option<u8> {
        if self.read_port(LINE_STATUS_REG) & LSR_RX_READY != 0 {
            Some(self.read_port(DATA_REG))
        } else {
            None
        }
    }

    /// Sends a string (byte slice) over the serial port.
    ///
    /// Converts `\n` to `\r\n` (CRLF) because serial terminals expect
    /// carriage return before newline to avoid the "staircase effect"
    /// where each line starts further to the right.
    pub fn write_string(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r'); // Carriage return first
            }
            self.write_byte(byte);
        }
    }

    // =========================================================================
    // Low-level I/O port access
    // =========================================================================
    //
    // x86 has a separate I/O address space (65536 ports, 0x0000-0xFFFF)
    // that is not part of the memory address space. It's accessed with
    // special IN/OUT instructions, not normal memory reads/writes.
    //
    // These are the only `unsafe` operations in this module.
    // =========================================================================

    /// Reads a byte from an I/O port.
    ///
    /// # Safety (handled internally)
    /// I/O port access is inherently unsafe because it can trigger hardware
    /// side effects. We confine this to this module where the port addresses
    /// are known and correct.
    #[inline]
    fn read_port(&self, offset: u16) -> u8 {
        let port = self.base + offset;
        let value: u8;
        // SAFETY: We're reading from a known UART register.
        // The port addresses (0x3F8-0x3FF) are standardized COM1 ports.
        unsafe {
            core::arch::asm!(
                "in al, dx",
                out("al") value,
                in("dx") port,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }

    /// Writes a byte to an I/O port.
    ///
    /// # Safety (handled internally)
    /// Same as read_port — I/O port access is unsafe but confined to
    /// known UART registers.
    #[inline]
    fn write_port(&self, offset: u16, value: u8) {
        let port = self.base + offset;
        // SAFETY: We're writing to a known UART register.
        // The port addresses (0x3F8-0x3FF) are standardized COM1 ports.
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("al") value,
                in("dx") port,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

/// Implements `core::fmt::Write` for SerialPort.
///
/// This lets us use Rust's `write!()` and `writeln!()` macros with the
/// serial port, which enables formatted output:
///   write!(serial, "Value: {:#x}", 42).unwrap();
///
/// This trait implementation is what makes our kprintln!() macro possible.
impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
