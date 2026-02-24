---
title: Home
layout: home
nav_order: 1
permalink: /
---

# MinimalOS NextGen

A **capability-based microkernel** operating system written from scratch in **Rust** for x86_64.
{: .fs-6 .fw-300 }

[Get Started]({% link getting-started.md %}){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[View on GitHub](https://github.com/example/MinimalOS){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## What is MinimalOS NextGen?

MinimalOS NextGen is an educational operating system that prioritizes **security** and **minimalism**. The kernel provides exactly six services — everything else runs in userspace:

| Service | Description |
|:--------|:------------|
| **Address space isolation** | Each process gets its own page tables |
| **Capability enforcement** | Unforgeable tokens control access to resources |
| **IPC message delivery** | Opaque bytes + capability transfers between processes |
| **CPU time multiplexing** | Tickless scheduler with per-core run queues |
| **Interrupt routing** | IRQs delivered to capability holders |
| **Memory grant transfers** | Zero-copy page sharing via capabilities |

Drivers, filesystems, networking, and GUI all run as unprivileged userspace processes communicating through IPC — the kernel never implements policy.

## Target Hardware

The primary target is the **HP 15-ay028tu** laptop:

- **CPU**: Intel Pentium N3710 — 4-core Airmont (Braswell), 1.6–2.56 GHz
- **RAM**: 8 GB DDR3L-1600
- **GPU**: Intel HD 405 (Gen8)
- **Storage**: SATA HDD / replaceable with SSD
- **Boot**: UEFI via Limine v8.6.0 bootloader

The OS also runs in QEMU with OVMF UEFI firmware for development and CI.

## Design Philosophy

- **Policy-free kernel** — The kernel enforces mechanisms, never policy. Scheduling policy, filesystem layout, network protocols — all live in userspace.
- **Capability-based security** — Every resource access requires an unforgeable capability token. No ambient authority, no root user.
- **Rust all the way** — The entire kernel and userspace are written in Rust, leveraging the type system and ownership model for memory safety.
- **Minimal trusted computing base** — Only ~22 syscalls. The smaller the kernel, the less surface area for bugs.

## Current Status

{: .note }
> MinimalOS NextGen is under active development. Sprints 1–2 are complete; Sprints 3–7 are planned.

| Sprint | Focus | Status |
|:-------|:------|:-------|
| [Sprint 1]({% link subsystems/boot.md %}) | Boot & Serial Output | ✅ Complete |
| [Sprint 2]({% link subsystems/memory.md %}) | Memory Management | ✅ Complete |
| [Sprint 3]({% link subsystems/interrupts.md %}) | Interrupts & Exceptions | 🔲 Planned |
| [Sprint 4]({% link subsystems/scheduler.md %}) | Processes & Scheduler | 🔲 Planned |
| [Sprint 5]({% link subsystems/capabilities.md %}) | Capabilities & IPC | 🔲 Planned |
| [Sprint 6]({% link subsystems/syscalls.md %}) | Syscall Interface & Userspace | 🔲 Planned |
| [Sprint 7]({% link subsystems/userspace.md %}) | Init Process & First Program | 🔲 Planned |

See the full [Roadmap]({% link roadmap.md %}) for details.

## Quick Start

```bash
# Clone and build
git clone https://github.com/example/MinimalOS.git
cd MinimalOS
make

# Create bootable ISO and run in QEMU
make run
```

See [Getting Started]({% link getting-started.md %}) for prerequisites and detailed instructions.
