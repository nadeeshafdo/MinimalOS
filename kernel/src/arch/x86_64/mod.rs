// =============================================================================
// MinimalOS NextGen — x86_64 Architecture HAL (Hardware Abstraction Layer)
// =============================================================================
//
// This module contains ALL hardware-specific code for x86_64. If we ever
// port to another architecture (aarch64, riscv64), we add a sibling module
// and the rest of the kernel doesn't change.
//
// DESIGN RULE: All `unsafe` in the kernel should be concentrated here.
// Higher-level kernel code (capability system, IPC, scheduler logic) should
// be safe Rust calling into safe abstractions defined here.
//
// This module provides:
//   serial.rs    — COM1 UART for debug I/O (the first thing that works)
//   cpu.rs       — CPU feature detection, control registers, HLT
//   boot.rs      — Limine boot protocol request/response handling
//
// Future additions:
//   gdt.rs       — Global Descriptor Table + TSS (Sprint 2)
//   idt.rs       — Interrupt Descriptor Table (Sprint 2)
//   interrupts.rs — Exception & IRQ dispatch (Sprint 2)
//   apic.rs      — Local APIC + I/O APIC (Sprint 2)
//   paging.rs    — Page table structures (Sprint 2)
//   context.rs   — CPU context save/restore (Sprint 3)
//   smp.rs       — AP core startup (Sprint 3)
// =============================================================================

pub mod serial;
pub mod cpu;
pub mod boot;

