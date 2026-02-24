---
title: Getting Started
layout: default
nav_order: 2
---

# Getting Started
{: .no_toc }

Build MinimalOS NextGen from source, create a bootable ISO, and run it in QEMU.
{: .fs-6 .fw-300 }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Prerequisites

### Rust Toolchain

MinimalOS uses Rust **nightly** (configured automatically via `rust-toolchain.toml`):

```bash
# Install rustup if you haven't already
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The nightly toolchain and rust-src component install automatically
# when you run cargo in the project directory
```

The toolchain file pins the exact nightly version and includes the `rust-src` component (required for building `core`/`alloc` for bare metal).

### System Packages

On Ubuntu/Debian:

```bash
sudo apt install xorriso qemu-system-x86 git make
```

On Fedora:

```bash
sudo dnf install xorriso qemu-system-x86 git make
```

On Arch Linux:

```bash
sudo pacman -S libisoburn qemu-system-x86 git make
```

| Package | Purpose |
|:--------|:--------|
| `xorriso` | Creates bootable ISO images |
| `qemu-system-x86` | x86_64 emulator for testing |
| `git` | Downloads the Limine bootloader |
| `make` | Orchestrates the build |

### UEFI Firmware (for QEMU)

QEMU needs OVMF firmware for UEFI boot:

```bash
# Ubuntu/Debian
sudo apt install ovmf

# The firmware is expected at /usr/share/qemu/OVMF.fd
# or /usr/share/OVMF/OVMF_CODE.fd
```

---

## Building

### Debug Build

```bash
make
```

Builds the kernel with debug symbols and minimal optimization for development. Dependencies are still optimized (`opt-level = 2`) for faster boot.

### Release Build

```bash
make release
```

Builds with full optimizations: `opt-level = 2`, link-time optimization (LTO), single codegen unit. Produces a smaller, faster kernel binary.

### Create Bootable ISO

```bash
make iso          # Debug ISO → target/minimalos-debug.iso
make iso-release  # Release ISO → target/minimalos-release.iso
```

The ISO creation process:
1. Downloads and compiles the Limine bootloader (cached in `target/limine-src/`)
2. Copies the kernel ELF binary and boot config into an ISO directory structure
3. Uses `xorriso` to create a hybrid BIOS/UEFI bootable ISO

---

## Running in QEMU

### Interactive (with display)

```bash
make run          # Debug build
make run-release  # Release build
```

This opens a QEMU window showing the framebuffer console. Serial output goes to the terminal.

### Headless (CI-friendly)

```bash
make run-headless
```

Boots without a display window. Serial output is captured to `target/serial-output.log`. QEMU auto-terminates after 5 seconds. Useful for automated testing.

### Manual QEMU Command

```bash
qemu-system-x86_64 \
    -M q35 \
    -m 512M \
    -cpu Goldmont \
    -smp 4 \
    -bios /usr/share/qemu/OVMF.fd \
    -cdrom target/minimalos-debug.iso \
    -serial stdio \
    -no-reboot
```

| Flag | Purpose |
|:-----|:--------|
| `-M q35` | Modern chipset (PCIe, AHCI) |
| `-m 512M` | 512 MB RAM (approximate N3710 test config) |
| `-cpu Goldmont` | Closest QEMU CPU model to Airmont |
| `-smp 4` | 4 CPU cores (matches N3710) |
| `-serial stdio` | Serial output to terminal |
| `-no-reboot` | Don't reboot on triple fault — hang for debugging |

---

## Expected Boot Output

A successful boot produces serial output similar to:

```
==========================================================
  MinimalOS NextGen v0.1.0
  Capability-based microkernel for x86_64
==========================================================

[boot] HHDM offset: 0xFFFF800000000000
[boot] Kernel physical base: 0x00200000
[boot] Kernel virtual base:  0xFFFFFFFF80200000
[boot] Kernel size:          XX KiB (Y pages)
[boot] Sections:
  .text:   ...
  .rodata: ...
  .data:   ...
  .bss:    ...

[boot] Physical memory map (N entries):
  ...

[init] Phase 3: Memory management
[pmm] XXXXX total frames, XXXX used, XXXXX free (498 MiB free)
[heap] Test allocation OK: [42, 1337, 3735928559] (heap used: 32 bytes)
[heap] After drop: 0 bytes used / 256 KiB total
[vmm] Page table infrastructure ready (CR3 switch deferred to Sprint 3)

==========================================================
  Sprint 2 complete — memory management initialized!
==========================================================
```

---

## Project Structure

```
MinimalOS/
├── .cargo/config.toml      # Build target & rustflags
├── .github/workflows/      # CI pipeline
├── boot/limine.conf        # Bootloader configuration
├── docs/                   # This documentation site
├── kernel/
│   ├── Cargo.toml          # Kernel crate dependencies
│   ├── linker.ld           # Memory layout linker script
│   └── src/
│       ├── main.rs          # Entry point (5-phase boot)
│       ├── arch/x86_64/     # CPU HAL (serial, cpu, boot)
│       ├── drivers/         # Framebuffer console
│       ├── memory/          # PMM, VMM, heap, addresses
│       ├── sync/            # Spinlock primitives
│       └── util/            # Logging, panic handler
├── Cargo.toml              # Workspace root
├── Makefile                # Build orchestration
└── rust-toolchain.toml     # Pinned nightly version
```

---

## Makefile Targets

| Target | Description |
|:-------|:------------|
| `make` | Build kernel (debug) |
| `make release` | Build kernel (release, LTO) |
| `make iso` | Build + create bootable ISO (debug) |
| `make iso-release` | Build + create bootable ISO (release) |
| `make run` | Build + ISO + boot in QEMU (debug) |
| `make run-release` | Build + ISO + boot in QEMU (release) |
| `make run-headless` | Headless QEMU boot, serial to file |
| `make clean` | Remove build artifacts |
| `make distclean` | Remove everything including Limine |
| `make help` | Show all targets |

---

## Continuous Integration

The project includes a GitHub Actions workflow (`.github/workflows/build.yml`) that:

1. Builds the kernel in both debug and release modes
2. Creates a bootable ISO
3. Uploads the ISO as a build artifact
4. On version tags (`v*`): creates a GitHub Release with the ISO and SHA256 checksum
