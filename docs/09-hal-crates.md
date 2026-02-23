# Hardware Abstraction Layer & Crates

## khal — Kernel Hardware Abstraction Layer (`crates/khal/`)

The `khal` crate provides low-level hardware access primitives. It is `#![no_std]` and exposes sub-modules as public re-exports.

### Module Map

| Module | File | Description |
|---|---|---|
| `serial` | `serial.rs` | COM1 (0x3F8) UART output |
| `port` | `port.rs` | x86 `in`/`out` port I/O wrappers |
| `pic` | `pic.rs` | Legacy 8259 PIC initialization + remapping |
| `apic` | `apic.rs` | Local APIC timer, EOI, spurious vector |
| `ioapic` | `ioapic.rs` | I/O APIC interrupt routing |
| `keyboard` | `keyboard.rs` | PS/2 keyboard (scancode → keypress) |
| `mouse` | `mouse.rs` | PS/2 mouse (3-byte packet protocol) |
| `ramdisk` | `ramdisk.rs` | Ramdisk pointer/length storage |

---

### Serial (`serial.rs`)

- Writes to **COM1** (port 0x3F8)
- `SerialPort::init()` — configures baud rate, 8N1, FIFO
- `SerialPort::write_byte(b)` — busy-waits on transmit-ready
- Implements `core::fmt::Write` for `write!()` macro integration
- Used by `klog` for all debug output

### Port I/O (`port.rs`)

Thin wrappers around `x86_64::instructions::port`:
```rust
pub fn inb(port: u16) -> u8
pub fn outb(port: u16, value: u8)
pub fn inw(port: u16) -> u16
pub fn outw(port: u16, value: u16)
```

### PIC (`pic.rs`)

- `init_pics()` — remaps IRQs to vectors 32-47, then **masks all** (replaced by I/O APIC)
- Standard 8259A cascade: master at 0x20, slave at 0xA0
- After init, PIC is disabled in favor of APIC

### Local APIC (`apic.rs`)

- **MMIO base**: read from MSR `IA32_APIC_BASE` (0x1B), mapped via HHDM
- `init_apic()`:
  1. Enable APIC (set bit 8 of spurious vector register)
  2. Set spurious vector to 0xFF
  3. Configure timer: periodic mode, vector 32, divider 16
  4. Initial count = 0x300000 (~3.1M, arbitrary tuning)
- `apic_eoi()` — write 0 to EOI register (vector 0xB0)
- `read_apic_id()` — reads APIC ID register (offset 0x20)

### I/O APIC (`ioapic.rs`)

- **MMIO base**: 0xFEC00000 (standard), mapped via HHDM
- `init_ioapic()`:
  1. Read max redirection entries
  2. Mask all entries initially
- `ioapic_route(irq, vector, apic_id)`:
  - Writes low 32 bits: vector + delivery mode
  - Writes high 32 bits: destination APIC ID
  - Used to route keyboard (IRQ 1 → vector 33) and mouse (IRQ 12 → vector 44)

### Keyboard (`keyboard.rs`)

- Uses `pc_keyboard` crate for scancode set 1 decoding
- `KEYBOARD: Mutex<Keyboard<...>>` — global instance
- `handle_keyboard_interrupt()`:
  1. Read scancode from port 0x60
  2. Decode via `pc_keyboard` → `DecodedKey`
  3. Push `InputEvent { kind: KeyPress/KeyRelease }` to `EventBuffer`
  4. Send EOI

### Mouse (`mouse.rs`)

- **PS/2 auxiliary device** on port 0x60/0x64
- `init_mouse()`:
  1. Enable auxiliary port (command 0xA8)
  2. Enable IRQ 12 (get/set compaq status byte)
  3. Send defaults (0xF6), enable data reporting (0xF4)
- `handle_mouse_interrupt()`:
  1. Read byte from port 0x60
  2. Accumulate 3-byte packet: [status, dx, dy]
  3. Apply sign extension from status bits
  4. Push `InputEvent { kind: Mouse, abs_x, abs_y }` to `EventBuffer`
  5. Clamp to screen bounds (hardcoded 1280×800)
  6. Send EOI

### Ramdisk (`ramdisk.rs`)

Simple storage for the ramdisk location:
```rust
static RAMDISK_ADDR: AtomicU64 = ...;
static RAMDISK_LEN: AtomicU64 = ...;
pub fn set_ramdisk(addr: u64, len: u64)
pub fn get_ramdisk() -> (u64, u64)
```
Set during boot from Limine's module response.

---

## klog — Kernel Logging (`crates/klog/`)

- `#![no_std]` logging via COM1 serial
- `klog!()` — primary log macro, writes to serial
- `klog_warn!()` — prefixed with `[WARN]`
- All output goes through `khal::serial::SERIAL`
- No log levels, no filtering — all messages printed
- Format: `[klog] message\n`

---

## kdisplay — Display Support (`crates/kdisplay/`)

- `#![no_std]` crate for framebuffer display
- `FbInfo` struct: stores framebuffer address, width, height, pitch, bpp
- PSF font loading and glyph rendering utilities
- Used by UI Server actor (via memory capability) and early boot console
