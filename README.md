# MinimalOS

MinimalOS is an educational, work‑in‑progress 64‑bit x86_64 operating system kernel written in **Rust**. This repository contains the higher‑half kernel, small kernel libraries, and a tiny userspace ramdisk used for testing and demos.

Important: detailed and versioned documentation (build steps, design notes, API reference and tutorials) is published on GitHub Pages — please consult the site instead of updating this README for frequently changing details:

https://nadeeshafdo.github.io/MinimalOS/

Quick start
-----------
- Clone the repository and build the project:

```bash
git clone https://github.com/nadeeshafdo/MinimalOS.git
cd MinimalOS
make
```

- Create a bootable ISO and run in QEMU:

```bash
make iso
make run		# or: make qemu-bios / make qemu-uefi
```

For full, up‑to‑date instructions see the GitHub Pages documentation linked above.

What this repository contains
----------------------------
- Kernel (no_std) and small support crates for framebuffer, HAL, and logging
- Tooling to build a hybrid BIOS+UEFI ISO (Limine-based)
- A tiny ramdisk with a minimal init and shell used for testing

Contributing
------------
See `QUESTS.md` and the site documentation for development goals, coding conventions, and how to get started.

License
-------
Educational project — free to use and modify.

References
----------
- Project docs: https://nadeeshafdo.github.io/MinimalOS/
- `QUESTS.md` — development roadmap
