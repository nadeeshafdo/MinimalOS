# Build System & Toolchain

## Toolchain

| File | Purpose |
|---|---|
| `rust-toolchain.toml` | Nightly `2025-01-01`, components: `rust-src`, `llvm-tools-preview` |
| Targets | `x86_64-unknown-none` (kernel), `wasm32-unknown-unknown` (actors) |

## Workspace (`Cargo.toml`)

```toml
[workspace]
resolver = "2"
members = ["kernel", "crates/*", "sdk/*", "actors/*"]

[profile.dev]
opt-level = 0

[profile.dev.package."*"]
opt-level = 3   # Dependencies always optimized
```

## Kernel Dependencies (`kernel/Cargo.toml`)

| Crate | Version | Purpose |
|---|---|---|
| `limine` | 0.5 | Bootloader protocol requests |
| `x86_64` | 0.15 | ISF types for interrupt handlers |
| `spin` | 0.9 | Spinlock mutex |
| `linked_list_allocator` | 0.10 | Kernel heap with proper dealloc |
| `tinywasm` | 0.8 (no_default, parser) | Wasm interpreter |
| `klog` | local | Kernel logging |
| `khal` | local | Hardware abstraction |

## Custom Targets

### `build/target-kernel.json`
- `llvm-target`: `x86_64-unknown-none-elf`
- `code-model`: `kernel` (higher half addressing)
- `panic-strategy`: `abort`
- `disable-redzone`: `true`
- `features`: `-mmx,-sse,-sse2,...,+soft-float` (no FPU in kernel)

### `build/target-user.json`
- Same as kernel but `code-model` is default, `disable-redzone` is false
- Currently unused — all actors are Wasm

## Linker Script (`build/linker.ld`)

```
ENTRY(_start)
. = 0xFFFFFFFF80000000;   /* Higher half base */
__kernel_start = .;
.text   → :text
.rodata → :rodata
.data   → :data  (includes Limine request markers)
.bss    → :data  (must be last in :data PHDR)
__kernel_end = .;
```

## Build Script (`kernel/build.rs`)

Adds `-Tlinker.ld` and sets the linker search path to `build/`.

## Makefile Targets

| Target | Description |
|---|---|
| `make kernel` | Build kernel via cargo |
| `make actor-vfs` | Build VFS Wasm actor → `ramdisk/vfs.wasm` |
| `make actor-ui-server` | Build UI Server Wasm actor → `ramdisk/ui_server.wasm` |
| `make actor-shell` | Build Shell Wasm actor → `ramdisk/shell.wasm` |
| `make ramdisk` | Build all actors + copy `font.psf` → `ramdisk.tar` |
| `make iso` | kernel + ramdisk + limine → bootable ISO |
| `make qemu` / `make run` | Boot ISO in QEMU (BIOS, 2G RAM, 4 CPUs) |
| `make qemu-uefi` | Boot in UEFI mode (requires OVMF) |
| `make qemu-debug` | Boot with `-d int,cpu_reset -no-reboot` |
| `make clean` | Remove build artifacts |

## Bootloader (`limine.conf`)

```
timeout: 0
/MinimalOS
    protocol: limine
    kernel_path: boot():/boot/kernel
    module_path: boot():/boot/ramdisk.tar
```

The ramdisk TAR is loaded as a Limine module, accessible via `ModuleRequest`.

## Build Flow

```
make iso
  ├── cargo build --package minimalos_kernel --target build/target-kernel.json
  ├── cargo build actors/vfs   → wasm32-unknown-unknown → ramdisk/vfs.wasm
  ├── cargo build actors/ui_server → ramdisk/ui_server.wasm
  ├── cargo build actors/shell → ramdisk/shell.wasm
  ├── cp assets/font.psf → ramdisk/font.psf
  ├── tar cf ramdisk.tar -C ramdisk .
  └── xorriso → ISO with Limine BIOS+UEFI boot
```
