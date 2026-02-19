---
layout: default
title: Userspace Guide
---

# Userspace Guide

## Overview

MinimalOS runs user programs in Ring 3 (unprivileged mode). User programs are
compiled as freestanding Rust `no_std` ELF binaries, packaged into a TAR
ramdisk, and loaded by the kernel at boot time or on demand via `sys_spawn`.

## Architecture

```
┌──────────────────────────────────────────────┐
│				  User Mode (Ring 3)		  │
│											  │
│   init.elf		  shell.elf				│
│   (PID 1)		   (PID 2)				  │
│											  │
│   0x400000		  0x500000				 │
│   ────────		  ────────				 │
│   Spawns shell,	 Interactive prompt,	  │
│   loops yielding	reads keyboard,		  │
│					 spawns programs		   │
├──────────────────────────────────────────────┤
│			  syscall / sysret				│
├──────────────────────────────────────────────┤
│			   Kernel (Ring 0)				│
│											  │
│   Scheduler · Memory · Filesystem · Drivers  │
└──────────────────────────────────────────────┘
```

## Building User Programs

### Crate Setup

Each user program is a separate Cargo binary crate in the `user/` directory:

```
user/
├── init/
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/main.rs
└── shell/
	├── Cargo.toml
	├── build.rs
	└── src/main.rs
```

### Cargo.toml

```toml
[package]
name = "shell"
version = "0.1.0"
edition = "2021"

[dependencies]
# No external dependencies — syscalls are inline assembly
```

### build.rs

Each user crate has a `build.rs` that configures the linker script:

```rust
fn main() {
	println!("cargo:rustc-link-arg=-Tbuild/linker-shell.ld");
	println!("cargo:rerun-if-changed=build/linker-shell.ld");
}
```

### Compilation

User programs are compiled with a separate target (`build/target-user.json`)
that differs from the kernel target:

| Property | Kernel | User |
|----------|--------|------|
| Code model | `kernel` | `small` |
| Red zone | disabled | enabled |
| Entry point | `_start` | `_start` |

The Makefile builds user programs automatically:

```bash
make user-init	# Build init
make user-shell   # Build shell
make kernel	   # Builds both user programs first, then the kernel
```

### RAMDisk Packaging

User ELF binaries are copied into the `ramdisk/` directory and archived:

```bash
make ramdisk
# Copies init.elf and shell.elf into ramdisk/
# Creates ramdisk.tar via: tar cf build/dist/ramdisk.tar -C ramdisk .
```

The ramdisk is included in the ISO as a Limine module and loaded into memory
at boot.

## The Init Process

**File:** `user/init/src/main.rs`

The init process is the first user-mode program launched by the kernel. Its job
is minimal:

1. Log a startup message via `sys_log`.
2. Spawn the shell via `sys_spawn("shell.elf")`.
3. Enter an infinite loop calling `sys_yield()`.

```rust
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
	sys_log("[init] Starting...\n");
	sys_spawn("shell.elf");
	loop { sys_yield(); }
}
```

Init runs at virtual address `0x400000` with its stack at `0x800000 + 4096`.

## The Shell

**File:** `user/shell/src/main.rs`

The shell is an interactive user-mode program that:

1. Displays a prompt (`MinimalOS> `).
2. Reads keyboard input character-by-character via `sys_read(0, ...)`.
3. Assembles characters into a command buffer.
4. Handles backspace (erases last character).
5. On Enter, parses and executes the command.

### Built-in Commands

| Command | Description |
|---------|-------------|
| `help` | Display available commands |
| `hello` | Print a greeting |
| `exit` | Terminate the shell |

Any unrecognised command prints an error message.

### Input Loop

The shell uses a polling pattern for keyboard input:

```rust
loop {
	let n = sys_read(0, &mut buf, 1);
	if n == 0 {
		sys_yield();  // No input — yield CPU
		continue;
	}
	// Process the character...
}
```

This avoids busy-waiting: when no keyboard input is available, the shell
yields the CPU so other processes can run.

### Display

The shell outputs text by writing characters one at a time via `sys_log`,
which sends them to the serial port. The kernel's keyboard interrupt handler
also echoes typed characters to the framebuffer console.

## ELF Loading

**File:** `kernel/src/fs/elf.rs`

### Supported Format

| Field | Requirement |
|-------|-------------|
| Magic | `\x7FELF` |
| Class | 64-bit (ELFCLASS64) |
| Endianness | Little-endian |
| Type | Executable (`ET_EXEC`) |
| Machine | x86_64 (`EM_X86_64`) |

### Loading Process

When `sys_spawn` or the boot sequence loads an ELF:

1. **Parse headers**: Validate the ELF magic, class, and machine type. Extract
   the entry point and program header table.

2. **Map segments**: For each `PT_LOAD` program header:
   - Calculate the page range (`p_vaddr` to `p_vaddr + p_memsz`).
   - Allocate physical frames via `pmm::alloc_frame()`.
   - Map pages with `USER_RW` flags via `paging::map_page()`.
   - Zero the pages (for BSS sections).
   - Copy file data (`p_filesz` bytes from `p_offset`).

3. **Allocate stack**: Map a 4 KiB page at `0x800000 + pid * 0x10000` with
   `USER_RW` flags. The stack pointer starts at the top of this page.

4. **Create process**: Build a `Process` struct with the ELF entry point,
   user RSP, current CR3, and a new 32 KiB kernel stack.

## TAR Filesystem

**File:** `kernel/src/fs/tar.rs`

The ramdisk uses the USTAR tar format:

### TAR Header (512 bytes)

| Offset | Size | Field |
|--------|------|-------|
| 0 | 100 | File name |
| 100 | 8 | File mode (octal) |
| 124 | 12 | File size (octal ASCII) |
| 156 | 1 | Type flag (`'0'` = file, `'5'` = directory) |
| 257 | 6 | Magic (`"ustar"`) |

### Interface

```rust
/// Iterator over TAR entries.
pub struct TarIter<'a> { ... }

/// A single TAR entry.
pub struct TarEntry<'a> {
	pub name: &'a str,
	pub size: usize,
	pub typeflag: u8,
	pub data: &'a [u8],
}

/// Find a file by name.
pub fn find_file<'a>(ramdisk: &'a RamDisk, name: &str) -> Option<TarEntry<'a>>
```

### Global RAMDisk Storage

**File:** `kernel/src/fs/ramdisk.rs`

The ramdisk pointer and size are stored globally using `spin::Once<RamDisk>`
so that `sys_spawn` can access it from the syscall handler:

```rust
pub fn init(base: *const u8, size: usize)
pub fn get() -> Option<&'static RamDisk>
```

## Address Space Layout (User Programs)

```
0x0000_0000_0040_0000  ┌──────────────────┐
					   │   init.elf		│  .text, .rodata, .data, .bss
0x0000_0000_0050_0000  ├──────────────────┤
					   │   shell.elf	   │  .text, .rodata, .data, .bss
					   ├──────────────────┤
					   │   (unmapped)	  │
0x0000_0000_0080_0000  ├──────────────────┤
					   │   User stacks	 │  4 KiB per process
					   │   PID 1: 0x800000 │  (grows downward)
					   │   PID 2: 0x810000 │
					   │   ...			 │
					   └──────────────────┘
```

## Writing a New User Program

1. Create a new crate: `user/myprogram/`

2. Add `Cargo.toml`:
   ```toml
   [package]
   name = "myprogram"
   edition = "2021"
   ```

3. Add `build.rs` pointing to a linker script (or reuse an existing one with a
   unique load address).

4. Write `src/main.rs`:
   ```rust
   #![no_std]
   #![no_main]

   #[no_mangle]
   pub extern "C" fn _start() -> ! {
	   // Your code here — use syscalls for I/O
	   loop { /* sys_yield() */ }
   }

   #[panic_handler]
   fn panic(_: &core::panic::PanicInfo) -> ! {
	   loop {}
   }
   ```

5. Add the binary to the Makefile's ramdisk target.

6. Spawn it from the shell or init via `sys_spawn("myprogram.elf")`.
