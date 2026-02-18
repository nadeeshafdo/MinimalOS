---
layout: default
title: MinimalOS — A Rust OS from Scratch
---

<style>
  .hero {
    text-align: center;
    padding: 2rem 0 1rem;
    border-bottom: 1px solid #e1e4e8;
    margin-bottom: 2rem;
  }
  .hero img { width: 80px; margin-bottom: 1rem; }
  .hero h1 { font-size: 2rem; font-weight: 700; margin: 0.5rem 0; color: #24292e; }
  .hero p.tagline { font-size: 1.1rem; color: #586069; margin: 0.5rem 0 1.5rem; }
  .badges { display: flex; gap: 0.5rem; justify-content: center; flex-wrap: wrap; margin-bottom: 1.5rem; }
  .badge { display: inline-block; padding: 0.25rem 0.75rem; border-radius: 9999px; font-size: 0.8rem; font-weight: 600; }
  .badge-purple { background: #ede9fe; color: #6d28d9; }
  .badge-blue { background: #dbeafe; color: #1d4ed8; }
  .badge-green { background: #d1fae5; color: #065f46; }
  .features { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1.25rem; margin: 2rem 0; }
  .feature { padding: 1.25rem; border: 1px solid #e1e4e8; border-radius: 8px; }
  .feature h3 { font-size: 0.95rem; margin: 0 0 0.5rem; }
  .feature p { font-size: 0.85rem; color: #586069; margin: 0; line-height: 1.5; }
  .doc-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1rem; margin: 1.5rem 0; }
  .doc-card { display: block; padding: 1rem 1.25rem; border: 1px solid #e1e4e8; border-radius: 8px; text-decoration: none; color: inherit; transition: border-color 0.15s; }
  .doc-card:hover { border-color: #6366f1; }
  .doc-card h4 { margin: 0 0 0.25rem; font-size: 0.95rem; color: #24292e; }
  .doc-card p { margin: 0; font-size: 0.8rem; color: #586069; }
  .section-title { font-size: 1.3rem; font-weight: 600; margin: 2.5rem 0 0.5rem; padding-bottom: 0.5rem; border-bottom: 1px solid #e1e4e8; }
  .quick-start pre { background: #f6f8fa; padding: 1rem; border-radius: 6px; overflow-x: auto; font-size: 0.85rem; }
  .quick-start code { font-size: 0.85rem; }
  .stats { display: flex; gap: 2rem; justify-content: center; margin: 1.5rem 0; flex-wrap: wrap; }
  .stat { text-align: center; }
  .stat .number { font-size: 1.5rem; font-weight: 700; color: #6366f1; }
  .stat .label { font-size: 0.8rem; color: #586069; }
</style>

<div class="hero">
  <img src="assets/logo.svg" alt="MinimalOS logo">
  <h1>MinimalOS</h1>
  <p class="tagline">A 64-bit x86_64 operating system kernel written in Rust from scratch</p>
  <div class="badges">
    <span class="badge badge-purple">Rust</span>
    <span class="badge badge-blue">x86_64</span>
    <span class="badge badge-green">v0.0.69</span>
  </div>
  <div class="stats">
    <div class="stat"><div class="number">69</div><div class="label">Achievements</div></div>
    <div class="stat"><div class="number">8</div><div class="label">Ranks Complete</div></div>
    <div class="stat"><div class="number">5</div><div class="label">Syscalls</div></div>
  </div>
</div>

<div class="features">
  <div class="feature">
    <h3>🧠 Memory Management</h3>
    <p>Bitmap PMM, 4-level paging VMM, and a linked-list kernel heap allocator. Full <code>alloc</code> support.</p>
  </div>
  <div class="feature">
    <h3>🚦 Preemptive Multitasking</h3>
    <p>Round-robin scheduler with APIC timer preemption, assembly context switching, and per-task kernel stacks.</p>
  </div>
  <div class="feature">
    <h3>🔒 User Mode</h3>
    <p>Ring 3 processes via <code>syscall</code>/<code>sysret</code>. GDT with user segments, TSS, and dynamic RSP0.</p>
  </div>
  <div class="feature">
    <h3>💾 Filesystem</h3>
    <p>USTAR tar archive loaded as a ramdisk. ELF binary parser loads user programs into separate address spaces.</p>
  </div>
  <div class="feature">
    <h3>⌨️ Interactive Shell</h3>
    <p>User-mode shell with keyboard input via <code>sys_read</code>, command parsing, and program spawning.</p>
  </div>
  <div class="feature">
    <h3>🎨 Framebuffer Console</h3>
    <p>Pixel-level rendering with bitmap fonts, color support, scrolling, and <code>kprint!</code> formatting macros.</p>
  </div>
</div>

<h2 class="section-title">📖 Documentation</h2>

<div class="doc-grid">
  <a class="doc-card" href="kernel_architecture">
    <h4>Kernel Architecture</h4>
    <p>Boot flow, module structure, GDT/TSS layout, linker scripts, and custom Rust targets.</p>
  </a>
  <a class="doc-card" href="memory_management">
    <h4>Memory Management</h4>
    <p>Physical frame allocator, virtual memory paging, kernel heap, and the HHDM mapping.</p>
  </a>
  <a class="doc-card" href="process_management">
    <h4>Process Management</h4>
    <p>Process control blocks, context switching, round-robin scheduling, and task lifecycle.</p>
  </a>
  <a class="doc-card" href="syscalls">
    <h4>Syscall Reference</h4>
    <p>Complete reference for all 5 system calls: log, exit, yield, spawn, and read.</p>
  </a>
  <a class="doc-card" href="userspace">
    <h4>Userspace Guide</h4>
    <p>Building user programs, the init process, interactive shell, and ELF loading.</p>
  </a>
  <a class="doc-card" href="drivers">
    <h4>Drivers</h4>
    <p>Framebuffer display, PS/2 keyboard, APIC timer, serial port, and the HAL crate.</p>
  </a>
  <a class="doc-card" href="development_guide">
    <h4>Development Guide</h4>
    <p>Build instructions, project layout, adding modules, QEMU testing, and debugging tips.</p>
  </a>
</div>

<h2 class="section-title">🚀 Quick Start</h2>

<div class="quick-start">

```bash
# Clone the repository
git clone https://github.com/paigeadelethompson/MinimalOS.git
cd MinimalOS

# Build and run (requires Rust nightly, QEMU, xorriso, git, make)
make iso
make run
```

The Rust toolchain (`nightly-2025-01-01`) is pinned in `rust-toolchain.toml` and will be installed automatically by `rustup`.

</div>

<h2 class="section-title">🗺️ Roadmap</h2>

| Rank | Focus | Status |
|------|-------|--------|
| I | **The Awakening** — Boot & Basics | ✅ Complete |
| II | **The Artist** — Graphics & Output | ✅ Complete |
| III | **The Reflexes** — Interrupts & CPU | ✅ Complete |
| IV | **The Mind** — Memory Management | ✅ Complete |
| V | **The Senses** — Input & Drivers | ✅ Complete |
| VI | **The Barrier** — User Mode & Syscalls | ✅ Complete |
| VII | **The Vault** — Storage & Files | ✅ Complete |
| VIII | **The Conductor** — Multitasking & IPC | ✅ Complete |
| IX | **The Network** — Data & Buses | 🔲 Next |

See the full quest tracker in [QUESTS.md](https://github.com/paigeadelethompson/MinimalOS/blob/main/QUESTS.md).

---

<p style="text-align:center;color:#586069;font-size:0.8rem;margin-top:2rem;">
  MinimalOS is an educational project — free to use and modify.<br>
  <a href="https://github.com/paigeadelethompson/MinimalOS">View on GitHub</a>
</p>
