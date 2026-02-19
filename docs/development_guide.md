---
layout: default
title: Development Guide
---

# Development Guide

## Prerequisites

| Tool | Purpose |
|------|---------|
| **Rust** | Nightly toolchain (`nightly-2025-01-01`) — installed via `rustup` |
| **QEMU** | x86_64 system emulator for testing |
| **xorriso** | ISO 9660 image creation |
| **Git** | Cloning Limine bootloader |
| **GNU Make** | Build orchestration |
| **tar** | RAMDisk archive creation |

The exact Rust toolchain is pinned in `rust-toolchain.toml` and includes the
`rust-src` and `llvm-tools-preview` components. Running any `cargo` command in
the workspace will automatically install the correct toolchain.

## Building

### Quick Start

```bash
git clone https://github.com/paigeadelethompson/MinimalOS.git
cd MinimalOS
make iso
make run
```

### Build Targets

| Command | Description |
|---------|-------------|
| `make` | Build the kernel (and user programs) |
| `make kernel` | Build the kernel (and user programs) |
| `make user-init` | Build only the init user program |
| `make user-shell` | Build only the shell user program |
| `make ramdisk` | Create the ramdisk.tar archive |
| `make iso` | Build everything and create a bootable ISO |
| `make run` | Build ISO and launch QEMU (BIOS mode) |
| `make qemu-bios` | Run in QEMU with BIOS boot |
| `make qemu-uefi` | Run in QEMU with UEFI boot (requires OVMF) |
| `make qemu-debug` | Run with interrupt logging and no-reboot |
| `make clean` | Remove build artifacts |
| `make distclean` | Remove build artifacts and cloned Limine |

### Build Flow

```
make iso
  │
  ├─ make user-init
  │	└─ cargo build --package init --target build/target-user.json
  │
  ├─ make user-shell
  │	└─ cargo build --package shell --target build/target-user.json
  │
  ├─ make kernel
  │	└─ cargo build --package minimalos_kernel --target build/target-kernel.json
  │
  ├─ make ramdisk
  │	├─ cp init.elf, shell.elf → ramdisk/
  │	└─ tar cf ramdisk.tar -C ramdisk .
  │
  ├─ make limine
  │	└─ git clone limine (v8.x-binary)
  │
  └─ xorriso → build/dist/minimalos.iso
```

## Project Layout

```
MinimalOS/
├── Cargo.toml				  # Workspace root
├── Makefile					# Build orchestration
├── limine.conf				 # Bootloader configuration
├── rust-toolchain.toml		 # Pinned Rust nightly toolchain
├── QUESTS.md				   # Achievement-based development tracker
│
├── build/
│   ├── linker.ld			   # Kernel linker script (0xFFFFFFFF80000000)
│   ├── linker-shell.ld		 # Shell linker script (0x500000)
│   ├── target-kernel.json	  # Custom Rust target for kernel
│   └── target-user.json		# Custom Rust target for userspace
│
├── kernel/					 # Kernel binary crate
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/
│	   ├── main.rs			 # Entry point (_start), boot sequence
│	   ├── arch/			   # x86_64 architecture code
│	   │   ├── mod.rs
│	   │   ├── gdt.rs		  # Global Descriptor Table
│	   │   ├── tss.rs		  # Task State Segment
│	   │   ├── idt.rs		  # IDT structures
│	   │   └── syscall.rs	  # syscall/sysret MSR setup, dispatcher
│	   ├── memory/			 # Memory management
│	   │   ├── mod.rs		  # Census, APIC MMIO mapping
│	   │   ├── pmm.rs		  # Bitmap physical frame allocator
│	   │   ├── paging.rs	   # 4-level page table management
│	   │   └── heap.rs		 # Linked-list kernel heap (GlobalAlloc)
│	   ├── task/			   # Process management
│	   │   ├── mod.rs
│	   │   ├── process.rs	  # PCB, scheduler, context_switch_asm
│	   │   ├── input.rs		# Keyboard ring buffer (256 bytes)
│	   │   └── usermode.rs	 # Ring 3 transition helpers
│	   ├── traps/			  # Interrupt handling
│	   │   ├── mod.rs
│	   │   ├── idt.rs		  # IDT init, IST configuration
│	   │   └── handlers.rs	 # Exception + IRQ handlers
│	   └── fs/				 # Filesystem
│		   ├── mod.rs
│		   ├── tar.rs		  # USTAR tar parser
│		   ├── elf.rs		  # ELF64 parser + loader
│		   └── ramdisk.rs	  # Global ramdisk storage
│
├── crates/					 # Kernel-space libraries (no_std)
│   ├── kdisplay/			   # Framebuffer graphics + text console
│   ├── khal/				   # HAL: ports, PIC, APIC, keyboard, serial
│   └── klog/				   # Serial logging (COM1)
│
├── sdk/
│   └── sys/					# Shared types (kernel ↔ userspace)
│
├── user/					   # User-mode programs (no_std)
│   ├── init/				   # First user process (spawns shell)
│   └── shell/				  # Interactive command shell
│
├── ramdisk/					# Files packaged into ramdisk.tar
│   ├── hello.txt			   # Test file
│   ├── init.elf				# (built by make)
│   └── shell.elf			   # (built by make)
│
└── docs/					   # Documentation (GitHub Pages)
```

## Workspace Crates

| Crate | Path | Description |
|-------|------|-------------|
| `minimalos_kernel` | `kernel/` | Kernel entry point and all core subsystems |
| `klog` | `crates/klog/` | Serial port logging (COM1) |
| `kdisplay` | `crates/kdisplay/` | Framebuffer display and text console |
| `khal` | `crates/khal/` | Hardware Abstraction Layer |
| `sys` | `sdk/sys/` | Shared types between kernel and userspace |
| `init` | `user/init/` | First user-mode process |
| `shell` | `user/shell/` | Interactive shell |

All crates are `#![no_std]`. The workspace uses `resolver = "2"`.

## Adding a New Kernel Module

1. Create a directory under `kernel/src/` (e.g., `kernel/src/net/mod.rs`).
2. Declare it in `kernel/src/main.rs` with `mod net;`.
3. Use `klog` for debug output and `khal` for hardware access.

## Adding a New Crate

1. Create a directory under `crates/` with `Cargo.toml` and `src/lib.rs`.
2. Mark it `#![no_std]`.
3. The workspace `Cargo.toml` uses `members = ["crates/*"]`, so it's picked
   up automatically.
4. Add it as a dependency in `kernel/Cargo.toml`:
   ```toml
   mycrate = { path = "../crates/mycrate" }
   ```

## Adding a New User Program

1. Create `user/myprogram/` with `Cargo.toml`, `build.rs`, and `src/main.rs`.
2. Create a linker script in `build/` with a unique load address.
3. Add a build target in the `Makefile`.
4. Add the ELF to the ramdisk target.
5. See the [Userspace Guide](userspace) for details.

## Testing with QEMU

### Standard Run

```bash
make run
```

QEMU is configured with:
- **Machine:** Q35 chipset
- **RAM:** 2 GiB
- **Boot:** CD-ROM with the ISO
- **Serial:** Redirected to stdio (see kernel logs in your terminal)

### Automated Testing

For CI or scripted testing:

```bash
make iso
timeout 30 qemu-system-x86_64 \
	-M q35 -m 2G \
	-cdrom build/dist/minimalos.iso \
	-serial file:/tmp/serial.log \
	-display none \
	-no-reboot

cat /tmp/serial.log
```

This runs QEMU headlessly for 30 seconds and captures serial output to a file.

### Debug Mode

```bash
make qemu-debug
```

Enables QEMU's interrupt logging (`-d int,cpu_reset`) and prevents automatic
reboot on triple fault (`-no-reboot`).

## Debugging Tips

- **Serial output** is your primary debugging tool. Use `klog::info!()`,
  `klog::debug!()`, etc. to trace execution.
- **QEMU monitor** (Ctrl+A, C) provides CPU state inspection.
- **Page faults** print the faulting address (CR2) and error code.
- **Triple faults** in debug mode show the CPU state at the point of failure.
- **Stack overflows** often manifest as page faults at very low addresses or
  as double faults. The IST1 stack ensures double faults are catchable.

## Versioning & Commits

The project uses achievement-based versioning:

```
v0.0.{achievement_count}
```

Each achievement is a single commit:

```bash
git add <file>
git commit -m "feat: achievement [NNN] completed — Title"
git tag v0.0.NNN
git push origin main
git push origin v0.0.NNN
```

Progress is tracked in [QUESTS.md](https://github.com/paigeadelethompson/MinimalOS/blob/main/QUESTS.md).

## Development Roadmap

| Rank | Focus | Achievements | Status |
|------|-------|-------------|--------|
| I | The Awakening — Boot & Basics | [001]–[008] | ✅ |
| II | The Artist — Graphics & Output | [009]–[017] | ✅ |
| III | The Reflexes — Interrupts & CPU | [018]–[026] | ✅ |
| IV | The Mind — Memory Management | [027]–[036] | ✅ |
| V | The Senses — Input & Drivers | [037]–[043] | ✅ |
| VI | The Barrier — User Mode & Syscalls | [044]–[052] | ✅ |
| VII | The Vault — Storage & Files | [053]–[060] | ✅ |
| VIII | The Conductor — Multitasking & IPC | [061]–[069] | ✅ |
| IX | The Network — Data & Buses | [070]–[073] | 🔲 |
