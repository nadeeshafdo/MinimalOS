# MinimalOS Documentation

Comprehensive technical documentation for the MinimalOS microkernel.

## Table of Contents

| # | Document | Description |
|---|---|---|
| 01 | [Overview](01-overview.md) | Project identity, design principles, directory structure |
| 02 | [Build System](02-build-system.md) | Toolchain, Cargo workspace, custom targets, Makefile, build flow |
| 03 | [Boot Sequence](03-boot-sequence.md) | All 22 steps from `_start` to first `do_schedule()` |
| 04 | [Memory](04-memory.md) | Address space layout, PMM, 4-level paging, heap architecture |
| 05 | [x86-64 Architecture](05-architecture-x86.md) | GDT, TSS, IDT, SMP, syscall entry |
| 06 | [Scheduling](06-scheduling.md) | Process PCB, context switch, round-robin scheduler, clock |
| 07 | [Capabilities & IPC](07-capabilities-ipc.md) | Capability engine, permissions, IPC messages, blocking semantics |
| 08 | [Wasm Runtime](08-wasm-runtime.md) | tinywasm integration, host functions, actor trampoline |
| 09 | [HAL & Crates](09-hal-crates.md) | Serial, APIC, I/O APIC, keyboard, mouse, klog |
| 10 | [Actors](10-actors.md) | VFS, UI Server, Shell — protocols and service loops |
| 11 | [Capability Map](11-capability-map.md) | Post-spawn slot assignments, IPC flow diagram |
| 12 | [Known Limitations](12-known-limitations.md) | Current issues and prioritized next steps |

## Quick Reference

- **Kernel entry**: `kernel/src/main.rs` → `_start()`
- **Scheduler**: `kernel/src/task/process.rs` → `do_schedule()`
- **Capabilities**: `kernel/src/cap.rs`
- **IPC**: `kernel/src/ipc.rs`
- **Wasm host functions**: `kernel/src/wasm.rs`
- **Actor SDK**: `actors/sdk/src/lib.rs`
- **Build**: `make run` (builds everything + launches QEMU)
