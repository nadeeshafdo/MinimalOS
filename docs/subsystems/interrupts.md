---
title: Interrupts & Exceptions
layout: default
parent: Subsystems
nav_order: 3
---

# Sprint 3 — Interrupts & Exceptions
{: .no_toc }

Handle CPU exceptions and hardware interrupts safely.
{: .fs-6 .fw-300 }

**Status**: 🔲 Planned
{: .label .label-yellow }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Overview

Sprint 3 enables the kernel to respond to CPU exceptions (page faults, general protection faults) and hardware interrupts (timer, keyboard). This is a prerequisite for the scheduler (Sprint 4) and for safely switching to our own page tables.

---

## GDT (Global Descriptor Table)

{: .todo }
> Not yet implemented.

The GDT defines memory segments for Ring 0 (kernel) and Ring 3 (user) code. On x86_64, segments are mostly legacy, but the GDT is still required for:

- **Kernel code segment** (Ring 0) — used by SYSCALL entry
- **Kernel data segment** (Ring 0)
- **User code segments** (Ring 3, both 64-bit and 32-bit compat)
- **User data segment** (Ring 3)
- **TSS (Task State Segment)** — per-core, defines:
  - `RSP0`: kernel stack pointer (used on privilege level change)
  - `IST1–7`: Interrupt Stack Table entries (dedicated stacks for critical exceptions)

### Planned Design

- One GDT per CPU core (for per-core TSS)
- Double fault handler gets its own IST stack (IST1) so it works even if the kernel stack overflows
- Loaded via `lgdt` instruction during early boot

---

## IDT (Interrupt Descriptor Table)

{: .todo }
> Not yet implemented.

The IDT maps interrupt/exception vectors (0–255) to handler functions.

### Exception Handlers (Vectors 0–31)

| Vector | Exception | Handler Plan |
|:-------|:----------|:------------|
| 0 | Divide Error | Print diagnostics, kill process |
| 6 | Invalid Opcode | Print diagnostics, kill process |
| 8 | Double Fault | IST1 stack, print registers, halt |
| 13 | General Protection Fault | Print error code + RIP, kill process |
| 14 | Page Fault | Detailed handler (see below) |

### Page Fault Handler (Vector 14)

The most important exception handler. On page fault, the CPU provides:
- **CR2**: the faulting virtual address
- **Error code**: why the fault occurred (not-present, write to read-only, user vs kernel, instruction fetch)

Planned behavior:
1. **Kernel fault** → panic with full register dump (bug in the kernel)
2. **User fault on valid VMA** → map the page (demand paging, copy-on-write)
3. **User fault on invalid address** → deliver signal / kill process

### IRQ Handlers (Vectors 32+)

After the LAPIC and I/O APIC are configured:
- **Vector 32**: LAPIC timer (used for preemptive scheduling)
- **Vector 33+**: I/O APIC routed interrupts (keyboard, serial, etc.)
- **Vector 255**: Spurious interrupt (LAPIC)

---

## LAPIC (Local APIC)

{: .todo }
> Not yet implemented.

Each CPU core has its own Local APIC for:
- **Timer interrupts** — calibrated one-shot timer for tickless scheduling
- **Inter-Processor Interrupts (IPI)** — used for SMP coordination
- **Interrupt priority** — Task Priority Register (TPR) for masking

### Timer Calibration

1. Program the PIT (Programmable Interval Timer) for a known interval
2. Start the LAPIC timer in free-running mode
3. Wait for the PIT interval to elapse
4. Read the LAPIC timer count → compute ticks per millisecond
5. Use one-shot mode: set the deadline, get exactly one interrupt, reprogram

### MMIO Access

The LAPIC is accessed via memory-mapped registers at a fixed physical address (typically `0xFEE00000`), accessed through the HHDM.

---

## I/O APIC

{: .todo }
> Not yet implemented.

The I/O APIC routes external hardware interrupts to CPU cores.

### MADT Parsing

The ACPI MADT (Multiple APIC Description Table) contains:
- I/O APIC base address and global interrupt base
- Interrupt Source Override entries (IRQ remapping, e.g., IRQ0 → GSI2)
- Local APIC entries for each CPU core (used in SMP init)

### Redirection Table

Each I/O APIC pin has a Redirection Table Entry (RTE) that specifies:
- **Destination CPU** (or broadcast)
- **Delivery mode** (fixed, lowest priority, NMI, etc.)
- **Trigger mode** (edge vs level)
- **Vector number** (which IDT entry to invoke)
- **Mask bit** (enable/disable)

### Legacy IRQ Routing

| IRQ | Device | I/O APIC Pin |
|:----|:-------|:-------------|
| 0 | PIT Timer | GSI 2 (typically remapped) |
| 1 | Keyboard | GSI 1 |
| 3 | COM2 | GSI 3 |
| 4 | COM1 | GSI 4 |
| 8 | RTC | GSI 8 |
| 12 | PS/2 Mouse | GSI 12 |

---

## Dependencies

- **Requires**: Sprint 2 (PMM for IST stack allocation, VMM for understanding page faults)
- **Enables**: Sprint 4 (scheduler needs timer interrupts, context switch needs IST)

---

## Deferred from Sprint 2

Two VMM features were deferred to Sprint 3 because they require exception handling:

1. **Kernel higher-half remap** — Create our own PML4, map the kernel with proper W^X permissions, switch from Limine's page tables
2. **W^X enforcement** — Set `.text` as Execute-only, `.rodata` as Read-only, `.data`/`.bss` as NX (No Execute)

These will be the first tasks in Sprint 3, immediately after IDT setup enables page fault debugging.
