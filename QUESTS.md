# MinimalOS NextGen — The Complete Quest Tracker

> **The Vision:** A mathematically isolated, Zero-Copy, Single Address Space OS (SASOS) built purely on WebAssembly and unforgeable capabilities.
> 
> **The Great Purge:** At `v0.0.88`, the legacy ELF monolithic architecture was completely eradicated. The timeline diverged.
>
> **The End Game:** The project ceases development when it achieves **The Singularity** (Rank XV) — self-hosting compilation and native web browsing.
>
> **How to play:** Complete a quest, mark it `[x]`, bump the version in `Cargo.toml`, and commit: `feat: quest [NNN] completed — Title`

---

## 🛡️ Rank I: The Awakening (Boot & Basics) ✅
*The fundamental physical reality of the hardware.*
- [x] [001] **The Spark:** Create `cargo init` and compile a blank `no_std` binary.
- [x] [002] **The Architect:** Write the `linker.ld` script to define memory sections.
- [x] [003] **The Protocol:** Create `limine.conf` and configure the Limine bootloader.
- [x] [004] **The Image:** Successfully build `minimalos.iso` using `xorriso`.
- [x] [005] **First Breath:** Boot QEMU and see the Limine menu.
- [x] [006] **The Handover:** Kernel `_start` is called (verified via serial output).
- [x] [007] **The Map:** Successfully read the `BootInfo` struct from Limine.
- [x] [008] **Panic Button:** Implement `#[panic_handler]` so the OS halts safely.

## ⚡ Rank II: The Reflexes (Interrupts & CPU) ✅
*Handling hardware events without crashing.*
- [x] [009] **The Gatekeeper:** Define the IDT (Interrupt Descriptor Table) struct.
- [x] [010] **The Loader:** Load the IDT using the `lidt` assembly instruction.
- [x] [011] **Trap Card:** Create a handler for "Breakpoint" (Int 3) and trigger it.
- [x] [012] **Safety Net:** Create a "Double Fault" handler with a separate stack (IST).
- [x] [013] **Silence the Old:** Remap and disable the legacy 8259 PIC.
- [x] [014] **Modern Times:** Enable the APIC (Advanced Programmable Interrupt Controller).
- [x] [015] **The Heartbeat:** Enable the Local APIC Timer.

## 🧠 Rank III: The Mind (Memory Management) ✅
*Moving from raw physical addresses to managed memory.*
- [x] [016] **The Census:** Iterate over the Limine Memory Map and calculate total RAM.
- [x] [017] **The Accountant:** Implement a Bitmap Allocator (Physical Memory Manager).
- [x] [018] **Mine!:** Successfully call `pmm_alloc_frame()`.
- [x] [019] **Return It:** Successfully call `pmm_free_frame()`.
- [x] [020] **Higher Plane:** Create the recursive Page Table structure.
- [x] [021] **The Mapper:** Write `map_page(virt, phys, flags)`.
- [x] [022] **The Translator:** Write `virt_to_phys(addr)`.

---
## 🩸 THE TIMELINE DIVERGENCE: MINIMALOS NEXTGEN 🩸
*The era of the Monolith is over. The era of the Microkernel begins.*
---

## 🔒 Rank IV: The Capability Engine ✅
*The unforgeable physics of the NextGen operating system.*
- [x] [023] **The Token:** Define the `Capability` struct and `ObjectKind` enum.
- [x] [024] **The Vault:** Implement the `CapTable` (64 fixed slots per process).
- [x] [025] **Anti-Ghost:** Implement generation-guarded composite handles (ABA protection).
- [x] [026] **The Mailbox:** Define the 48-byte cache-line friendly `Message` struct.
- [x] [027] **The Queue:** Implement the `IpcQueue` 16-slot ring buffer.
- [x] [028] **The Router:** Implement `internal_cap_send()` for endpoint resolution.
- [x] [029] **The Narrowing:** Implement inline capability transfer with explicit permission narrowing.

## 🕸️ Rank V: The Wasm SASOS ✅
*Mathematical Software Fault Isolation inside a Single Address Space.*
- [x] [030] **The Interpreter:** Integrate `tinywasm` into Ring 0.
- [x] [031] **The Bridge:** Write the `build_imports()` host bridge for `env.*` syscalls.
- [x] [032] **The Trampoline:** Implement `wasm_actor_trampoline` (drop locks, sti, enter wasm).
- [x] [033] **TLB Annihilation:** Deprecate Ring 3 user page tables; assign kernel CR3 to all actors.
- [x] [034] **The Blitter:** Implement `SYS_CAP_MEM_WRITE` for zero-overhead physical DMA.
- [x] [035] **The Reclaimer:** Implement `linked_list_allocator` to prevent Wasm AST memory leaks.

## 🔪 Rank VI: The Great Purge ✅
*Burning down the legacy 1970s architecture.*
- [x] [036] **Zero-Allocation FS:** Write `vfs.wasm` to parse TAR files via cap blitting.
- [x] [037] **The Decoupling:** Delete `kernel/src/fs/`. The kernel no longer knows what a file is.
- [x] [038] **The Blind Blit:** Write `ui_server.wasm` to draw a white square to the Framebuffer.
- [x] [039] **The Blindness:** Delete `window.rs` and `kdisplay`. The kernel no longer knows what a pixel is.
- [x] [040] **The Annihilation:** Delete `user/`, `elf.rs`, `SYS_SPAWN`, and `iretq` logic. OS is 100% Wasm.
- [x] [041] **The Capability Shell:** Write `shell.wasm` to read files via IPC delegation.

---
## 🚀 THE FRONTIER (Active Quests) 🚀
---

## 🌪️ Rank VII: The Chaos Engine (True SMP) 🚧
*Unleashing the full physical silicon.*
- [ ] [042] **The Awakening:** Initialize the local APIC timer on all Application Processors (Cores 1-3).
- [ ] [043] **The Maelstrom:** Command all APs to drop `hlt` and enter `do_schedule()` concurrently.
- [ ] [044] **State Machine:** Implement the 2-state INIT/READY architecture in `ui_server.wasm` to survive SMP races.

## 🎨 Rank VIII: The Compositor (UI & Rendering)
*Drawing the world from the sandbox.*
- [ ] [045] **The Font:** `vfs.wasm` successfully reads `font.psf` and delegates memory to UI Server.
- [ ] [046] **First Word:** The UI Server parses PSF glyphs and blits "MinimalOS" to the Framebuffer.
- [ ] [047] **The Window:** UI Server implements a virtual window capability and delegates it to the Shell.
- [ ] [048] **The Compositor:** UI Server merges overlapping Wasm memory buffers into the final Framebuffer natively.

## 📡 Rank IX: The Senses (Input Routing)
*Talking to the real world using hardware capabilities.*
- [ ] [049] **The IRQ Token:** Implement `SYS_CAP_IRQ_WAIT` allowing actors to block on hardware interrupts.
- [ ] [050] **The Typist:** Write `ps2_keyboard.wasm`. Grant it `ObjectKind::IrqLine { 1 }` and `ObjectKind::IoPort { 0x60 }`.
- [ ] [051] **The Terminal:** Route keyboard IPC events from the Typist actor to the Shell actor.
- [ ] [052] **The Pointer:** Write `ps2_mouse.wasm`. Route IRQ 12. Send coordinates to the UI Server to draw a cursor.

## ⚡ Rank X: Speed of Light (AOT/JIT Compilation)
*Breaking the interpreter speed limits.*
- [ ] [053] **The Profiler:** Write an actor to calculate millions of primes; measure the slow execution of `tinywasm`.
- [ ] [054] **The Engine Swap:** Replace `tinywasm` in the kernel with an AOT (Ahead-of-Time) or JIT compiler adapted for `no_std` (e.g., cranelift or modified wasmtime).
- [ ] [055] **Native Blit:** Wasm actors now execute as raw x86_64 machine code while trapped in software fault isolation.

## 🖧 Rank XI: The Oracle (Hardware Buses)
*Discovering the bare metal.*
- [ ] [056] **PCIe Cap:** Create a capability for the PCIe Configuration Space (MMCFG).
- [ ] [057] **The Enumerator:** Write `pcie.wasm` to scan the bus and log all connected hardware vendors and devices.
- [ ] [058] **Dynamic Routing:** Allow the kernel to dynamically mint `Memory` capabilities for PCIe Base Address Registers (BARs) and grant them to specific drivers.
- [ ] [059] **The Universal Bus:** Write `usb_xhci.wasm` to act as the master USB controller.

## 💾 Rank XII: The Leviathan (Real Storage)
*True persistence beyond the RAMDisk.*
- [ ] [060] **The Storage Controller:** Write an NVMe PCIe driver actor using BAR capabilities.
- [ ] [061] **Sector Routing:** Route block-read/write IPC from the NVMe driver to the VFS.
- [ ] [062] **The Real FS:** Expand `vfs.wasm` to understand FAT32 or Ext4, replacing the immutable TAR archive.
- [ ] [063] **The Writer:** Successfully create, write, and save a text file to a physical hard drive.

## 🌐 Rank XIII: The Web (Networking)
*Connecting to the outside world.*
- [ ] [064] **The NIC:** Write a VirtIO-Net or Intel E1000 actor driver.
- [ ] [065] **The Stack:** Implement a TCP/IP stack actor (e.g., porting `smoltcp` to Wasm).
- [ ] [066] **The Socket:** Define `Socket` capabilities.
- [ ] [067] **Hello World Wide Web:** Shell actor successfully resolves a DNS query and sends an HTTP GET request.

## 🎭 Rank XIV: The Illusion (WASI Compatibility)
*Running standard software without rewriting it.*
- [ ] [068] **The Translator:** Write `wasi_server.wasm`. It accepts standard WASI system calls from other actors and translates them into MinimalOS Capability IPC messages.
- [ ] [069] **The Imports:** Run a standard, unmodified Wasm program compiled with standard Rust `std` by routing it through the WASI server.
- [ ] [070] **The Ecosystem:** Run Python (compiled to Wasm-WASI) directly inside the MinimalOS shell.

## 🌌 Rank XV: The Singularity (The Final Horizon)
*The system sustains itself.*
- [ ] [071] **The Editor:** Port a command-line text editor (like `vim` or `nano`) to Wasm and run it in the UI Server.
- [ ] [072] **The Compiler:** Port `rustc` and `cargo` to WebAssembly via the WASI layer.
- [ ] [073] **Self-Hosting:** Compile the MinimalOS kernel source code *inside* the MinimalOS environment.
- [ ] [074] **The Browser:** Port a graphical WebAssembly-compatible browser engine (like Servo).
- [ ] [075] **End of Development:** MinimalOS is feature-complete. It is a mathematically secure, graphical, network-connected capability OS capable of developing its own next iteration.