// =============================================================================
// init — The God Process (PID 1)
// =============================================================================
//
// This is the first userspace process spawned by the MinimalOS kernel.
// It runs at Ring 3 (CPL=3) with absolute authority over the system,
// granted via capability slots:
//
//   Slot 1: PmmAllocator                     — mint physical frames
//   Slot 2: IoPort { base: 0x3F8, size: 8 }  — direct COM1 serial output
//   Slot 3: Process { pid: 1 } (self)        — SYS_MAP_MEMORY on own space
//
// The kernel maps the initrd TarFS pages at virtual address 0x1000_0000
// (read-only) so Init can parse the archive from Ring 3.
//
// SPRINT 10, PHASE 1 PROVES:
//   1. Init boots the Ring 3 global allocator (linked_list_allocator)
//   2. Uses capability-gated SYS_ALLOC_MEMORY + SYS_MAP_MEMORY to build heap
//   3. Constructs a Vec<u64> in Ring 3 — proving dynamic allocation works
//   4. Retains all Sprint 9 proofs (TarFS parsing, memory management)
//
// =============================================================================

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

// =============================================================================
// Constants — Capability Slot Layout
// =============================================================================

/// CNode slot 1: PmmAllocator — allocate physical frames.
const PMM_SLOT: u64 = 1;

/// CNode slot 2: IoPort capability for COM1 (0x3F8, size 8).
const IO_SLOT: u64 = 2;

/// CNode slot 3: Process capability (self, PID 1) — for SYS_MAP_MEMORY.
const SELF_PROC_SLOT: u64 = 3;

/// COM1 data register (Transmit Holding / Receive Buffer).
const COM1_DATA: u16 = 0x3F8;

/// COM1 Line Status Register.
const COM1_LSR: u16 = 0x3FD;

/// LSR bit 5: Transmit Holding Register Empty.
const LSR_TX_EMPTY: u8 = 1 << 5;

/// Virtual address where the kernel maps the initrd TarFS pages.
const INITRD_BASE: usize = 0x1000_0000;

/// CNode scratch slot for dynamic frame allocation (reused each iteration).
const SCRATCH_SLOT: u64 = 10;

/// Virtual base address for the Ring 3 heap.
const HEAP_BASE: u64 = 0x4000_0000;

/// Number of 4 KiB pages for the Ring 3 heap (100 pages = 400 KiB).
const HEAP_PAGES: u64 = 100;

// =============================================================================
// Init Entry Point
// =============================================================================

/// Entry point — the kernel IRETQ's here into Ring 3.
///
/// Runs at virtual address 0x400000 (set by linker.ld).
/// All hardware access goes through libmnos syscall wrappers.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    // =========================================================================
    // Phase 1: Hello from Ring 3
    // =========================================================================
    print_str(b"\r\n");
    print_str(b"==========================================================\r\n");
    print_str(b"  [init] The God Process (PID 1) -- Ring 3\r\n");
    print_str(b"  Sprint 10 Phase 1: Ring 3 Global Allocator\r\n");
    print_str(b"==========================================================\r\n");
    print_str(b"\r\n");

    // =========================================================================
    // Phase 2: Parse TarFS from mapped initrd pages
    // =========================================================================
    print_str(b"[init] Parsing initrd TarFS at 0x1000_0000...\r\n");

    let initrd = unsafe {
        core::slice::from_raw_parts(INITRD_BASE as *const u8, 4 * 1024 * 1024)
    };

    let file_count = tar_list(initrd);
    print_str(b"[init] Found ");
    print_dec(file_count);
    print_str(b" file(s) in initrd\r\n");

    // =========================================================================
    // Phase 3: Bootstrap Ring 3 Heap
    // =========================================================================
    print_str(b"\r\n[init] Bootstrapping Ring 3 heap...\r\n");
    print_str(b"[init]   heap_base=0x4000_0000, pages=100 (400 KiB)\r\n");
    print_str(b"[init]   alloc_slot=1 (PmmAllocator), proc_slot=3 (self)\r\n");

    libmnos::heap::init_heap(HEAP_BASE, HEAP_PAGES, PMM_SLOT, SELF_PROC_SLOT, SCRATCH_SLOT);

    print_str(b"[init]   OK: Heap initialized (400 KiB at 0x4000_0000)\r\n");

    // =========================================================================
    // Phase 4: Prove Vec allocation in Ring 3
    // =========================================================================
    print_str(b"\r\n[init] Proving Vec<u64> allocation in Ring 3...\r\n");

    let mut vec: Vec<u64> = Vec::new();
    vec.push(0xDEAD_BEEF);
    vec.push(0xCAFE_BABE);
    vec.push(0x1337_C0DE);

    print_str(b"[init]   vec.len() = ");
    print_dec(vec.len() as u64);
    print_str(b"\r\n");

    for (i, &val) in vec.iter().enumerate() {
        print_str(b"[init]   vec[");
        print_dec(i as u64);
        print_str(b"] = ");
        print_hex(val);
        print_str(b"\r\n");
    }

    // Validate values are correct
    assert_eq!(vec[0], 0xDEAD_BEEF);
    assert_eq!(vec[1], 0xCAFE_BABE);
    assert_eq!(vec[2], 0x1337_C0DE);

    print_str(b"[init]   OK: All values verified!\r\n");

    // Prove we can grow the vector (forces reallocation)
    print_str(b"\r\n[init] Proving Vec growth (reallocation)...\r\n");
    for i in 0..100u64 {
        vec.push(i);
    }
    print_str(b"[init]   vec.len() = ");
    print_dec(vec.len() as u64);
    print_str(b" (after pushing 100 more)\r\n");
    print_str(b"[init]   vec.capacity() = ");
    print_dec(vec.capacity() as u64);
    print_str(b"\r\n");
    print_str(b"[init]   OK: Vector reallocation succeeded!\r\n");

    // Drop the vector to prove the allocator handles deallocation
    drop(vec);
    print_str(b"[init]   OK: Vec dropped (memory returned to allocator)\r\n");

    // =========================================================================
    // Victory Banner
    // =========================================================================
    print_str(b"\r\n");
    print_str(b"==========================================================\r\n");
    print_str(b"  [init] SUCCESS: Ring 3 Global Allocator PROVEN!\r\n");
    print_str(b"  [init]   - 100 pages mapped via capability syscalls\r\n");
    print_str(b"  [init]   - linked_list_allocator initialized (400 KiB)\r\n");
    print_str(b"  [init]   - Vec<u64> constructed, grown, and dropped\r\n");
    print_str(b"  [init]   - Dynamic allocation works in Ring 3!\r\n");
    print_str(b"  [init] Sprint 10 Phase 1 COMPLETE.\r\n");
    print_str(b"==========================================================\r\n");

    halt_loop();
}

// =============================================================================
// Serial I/O Helpers
// =============================================================================

/// Writes a single byte to COM1 using the polled TX path.
#[inline(always)]
fn write_byte(byte: u8) {
    // Wait for Transmit Holding Register Empty
    loop {
        match libmnos::io::sys_port_in(IO_SLOT, COM1_LSR) {
            Ok(lsr) if lsr & LSR_TX_EMPTY != 0 => break,
            Ok(_) => {}
            Err(_) => return,
        }
    }
    let _ = libmnos::io::sys_port_out(IO_SLOT, COM1_DATA, byte);
}

/// Prints a byte string to COM1.
fn print_str(s: &[u8]) {
    for &b in s {
        write_byte(b);
    }
}

/// Prints a u64 in decimal to COM1.
fn print_dec(mut n: u64) {
    if n == 0 {
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        write_byte(buf[i]);
    }
}

/// Prints a u64 in hexadecimal (0x...) to COM1.
fn print_hex(n: u64) {
    print_str(b"0x");
    let hex = b"0123456789ABCDEF";
    let mut started = false;
    for shift in (0..16).rev() {
        let nibble = ((n >> (shift * 4)) & 0xF) as usize;
        if nibble != 0 || started || shift == 0 {
            write_byte(hex[nibble]);
            started = true;
        }
    }
}

// =============================================================================
// USTAR TarFS Parser (Ring 3)
// =============================================================================
//
// Scans the USTAR TAR archive mapped at INITRD_BASE. Lists all files.
// Stops at the first block of all zeros (standard USTAR EOF marker).

/// USTAR magic bytes at offset 257 in the header.
const USTAR_MAGIC: &[u8; 5] = b"ustar";

/// Lists all files in a USTAR TAR archive. Returns the count.
fn tar_list(data: &[u8]) -> u64 {
    let mut offset = 0usize;
    let mut count = 0u64;

    while offset + 512 <= data.len() {
        let header = &data[offset..offset + 512];

        // Check for end-of-archive (all-zero block)
        if header.iter().all(|&b| b == 0) {
            break;
        }

        // Validate USTAR magic (offset 257, 5 bytes)
        if &header[257..262] != USTAR_MAGIC {
            print_str(b"[init]   WARNING: non-USTAR block at offset ");
            print_hex(offset as u64);
            print_str(b"\r\n");
            break;
        }

        // Extract filename (offset 0, null-terminated, max 100 bytes)
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = &header[..name_end];

        // Extract size from octal ASCII at offset 124, 12 bytes
        let size = parse_octal(&header[124..136]);

        print_str(b"[init]   ");
        print_str(name);
        print_str(b" (");
        print_dec(size);
        print_str(b" bytes)\r\n");

        count += 1;

        // Advance past this header + file data (rounded up to 512-byte blocks)
        let data_blocks = (size as usize + 511) / 512;
        offset += 512 + data_blocks * 512;
    }

    count
}

/// Parses an octal ASCII string (USTAR size field).
fn parse_octal(field: &[u8]) -> u64 {
    let mut result = 0u64;
    for &b in field {
        if b == 0 || b == b' ' {
            break;
        }
        if b >= b'0' && b <= b'7' {
            result = result * 8 + (b - b'0') as u64;
        }
    }
    result
}

// =============================================================================
// Utility
// =============================================================================

/// Infinite loop — Init is done. In a future sprint this would idle-loop
/// waiting for child process events.
fn halt_loop() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// =============================================================================
// Panic Handler
// =============================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Try to print something useful
    print_str(b"\r\n[init] PANIC!\r\n");
    loop {
        core::hint::spin_loop();
    }
}
