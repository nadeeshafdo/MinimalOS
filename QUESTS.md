# MinimalOS Quest Tracker

> **Versioning:** `v0.0.{AchievementCount}` — current: **`v0.0.69`**
>
> Start: `v0.0.0` → Goal: `v1.0.0` (~100 micro-tasks)
>
> **How to play:** Complete a quest, mark it `[x]`, bump the version in
> `Cargo.toml`, and commit: `feat: achievement [NNN] completed — Title`

---

## 🛡️ Rank I: The Awakening (Boot & Basics) ✅

*Focus: Getting the kernel to run on the metal without crashing.*

- [x] [001] **The Spark:** Create `cargo init` and compile a blank `no_std` binary.
- [x] [002] **The Architect:** Write the `linker.ld` script to define memory sections.
- [x] [003] **The Protocol:** Create `limine.conf` and configure the Limine bootloader.
- [x] [004] **The Image:** Successfully build `minimalos.iso` using `xorriso`.
- [x] [005] **First Breath:** Boot QEMU and see the Limine menu.
- [x] [006] **The Handover:** Kernel `_start` is called (verified via serial output).
- [x] [007] **The Map:** Successfully read the `BootInfo` struct from Limine (Framebuffer ptr).
- [x] [008] **Panic Button:** Implement `#[panic_handler]` so the OS halts safely instead of rebooting.

## 🎨 Rank II: The Artist (Graphics & Output) ✅

*Focus: We have no VGA text mode. We must paint every pixel.*

- [x] [009] **The Canvas:** Extract the Framebuffer address, width, height, and pitch from `BootInfo`.
- [x] [010] **First Pixel:** Write a white pixel to `(100, 100)` by directly modifying memory.
- [x] [011] **The Screen Wipe:** Write a function to fill the entire screen with a specific color.
- [x] [012] **The Glyph:** Create a byte-array representing the letter 'A' (bitmap font).
- [x] [013] **The Typesetter:** Write a function `draw_char(x, y, char, color)`.
- [x] [014] **Hello World:** Render the string "Hello MinimalOS" to the screen.
- [x] [015] **The Logger:** Implement a `kprint!` macro that uses the framebuffer.
- [x] [016] **Scrollbar:** When text hits the bottom of the screen, shift the entire buffer up (software scrolling).
- [x] [017] **Formatting:** Support Rust formatting `kprintln!("Value: {:x}", 0xDEADBEEF)`.

## ⚡ Rank III: The Reflexes (Interrupts & CPU) ✅

*Focus: Handling hardware events without crashing.*

- [x] [018] **The Gatekeeper:** Define the IDT (Interrupt Descriptor Table) struct.
- [x] [019] **The Loader:** Load the IDT using the `lidt` assembly instruction.
- [x] [020] **Trap Card:** Create a handler for "Breakpoint" (Int 3) and trigger it successfully.
- [x] [021] **Safety Net:** Create a "Double Fault" handler with a separate stack (IST).
- [x] [022] **Silence the Old:** Remap and disable the legacy 8259 PIC.
- [x] [023] **Modern Times:** Enable the APIC (Advanced Programmable Interrupt Controller).
- [x] [024] **The Heartbeat:** Enable the Local APIC Timer.
- [x] [025] **Tick Tock:** Print a dot `.` to the screen every time the timer fires (e.g., 100Hz).
- [x] [026] **No Red Zone:** Verify compilation flags disable the "Red Zone" (critical for interrupt safety).

## 🧠 Rank IV: The Mind (Memory Management) ✅

*Focus: Moving from raw addresses to managed memory.*

- [x] [027] **The Census:** Iterate over the Limine Memory Map and calculate total RAM in bytes.
- [x] [028] **The Accountant:** Implement a Bitmap Allocator (Physical Memory Manager).
- [x] [029] **Mine!:** Successfully call `pmm_alloc_frame()` and get a valid address.
- [x] [030] **Return It:** Successfully call `pmm_free_frame()` and update the bitmap.
- [x] [031] **Higher Plane:** Create the recursive Page Table structure.
- [x] [032] **The Mapper:** Write `map_page(virt, phys, flags)`.
- [x] [033] **The Translator:** Write `virt_to_phys(addr)`.
- [x] [034] **The Heap:** Initialize the `GlobalAlloc` trait.
- [x] [035] **Dynamic Power:** Successfully use `Box::new(10)` inside the kernel.
- [x] [036] **Vectorization:** Successfully push an item to a `Vec<i32>`.

## ⌨️ Rank V: The Senses (Input & Drivers) ✅

*Focus: Interacting with the user.*

- [x] [037] **Port IO:** Implement `inb` and `outb` wrappers for x86 port communication.
- [x] [038] **PS/2 Controller:** Read the status register of the PS/2 port.
- [x] [039] **Key Down:** Receive a raw scancode when a key is pressed.
- [x] [040] **Decoder:** Create a match statement to convert Scancodes to ASCII characters.
- [x] [041] **The Echo:** Type on the keyboard and see letters appear on the MinimalOS screen.
- [x] [042] **Backspace:** Implement backspace logic in the `kprint!` buffer.
- [x] [043] **Serial Killer:** Implement a driver for the COM1 Serial Port (logging to host).

---

## 🔒 Rank VI: The Barrier (User Mode & Syscalls) ✅

*Focus: Running code that cannot crash the kernel.*

- [x] [044] **The Partition:** Define GDT User Code (64-bit) and User Data segments (Ring 3).
- [x] [045] **The TSS:** Setup the Task State Segment and load the Task Register (TR).
- [x] [046] **The Hotline:** Enable the `syscall` instruction via EFER MSR.
- [x] [047] **The Handler:** Implement the `syscall_handler` assembly stub (save/restore regs).
- [x] [048] **The Bridge:** Write the Rust `handle_syscall()` dispatcher.
- [x] [049] **The Drop:** Craft a `TrapFrame` to manually switch CPU ring from 0 to 3.
- [x] [050] **First Contact:** Successfully execute a trivial instruction (like `nop`) in Ring 3.
- [x] [051] **The Call:** Execute a `syscall` from Ring 3 and return to Ring 3 safely.
- [x] [052] **The Payload:** Load a raw binary from memory and run it as a user process.

## 💾 Rank VII: The Vault (Storage & Files) ✅

*Focus: Persistence. We need to read files.*

- [x] [053] **The Disk:** Detect the hard drive (RAMDisk / Tarball).
- [x] [054] **The Block:** Read a raw sector from the disk.
- [x] [055] **The Structure:** Define a TAR filesystem parser.
- [x] [056] **The Listing:** Implement `ls` to list files in the root directory.
- [x] [057] **The Reader:** Implement `cat` to print file contents to the screen.
- [x] [058] **The Loader:** Update the process manager to load ELF files from disk.
- [x] [059] **The Exec:** Successfully execute a user program (`init.elf`) from the TAR disk.
- [x] [060] **The Split:** Separate kernel and user builds (Cargo Workspace).

---

## 🚦 Rank VIII: The Conductor (Multitasking & IPC) ✅

*Focus: Running multiple things at once and letting them talk.*

- [x] [061] **The Context:** Define a `ProcessControlBlock` (PCB) to store register state (RIP, RSP, CR3).
- [x] [062] **The Switch:** Implement `context_switch(old_rsp, new_rsp)` in assembly.
- [x] [063] **The Scheduler:** Create a Round-Robin scheduler list (VecDeque of Tasks).
- [x] [064] **The Slice:** Hook the APIC Timer interrupt to call `schedule()`.
- [x] [065] **The Yield:** Implement `sys_yield()` so a process can give up CPU voluntarily.
- [x] [066] **The Spawn:** Implement `sys_spawn(path)` to launch a *second* process from the first.
- [x] [067] **The Exit:** Implement `sys_exit()` to clean up resources (zombie processes).
- [x] [068] **The Keyboard:** Buffer keyboard input in the kernel and implement `sys_read(STDIN)`.
- [x] [069] **The Shell:** Create `user/shell` that reads keyboard input and spawns programs found on disk.

## ✅ Rank IX: The Network (Data & Buses)

*Focus: Moving data between processes and hardware.*

- [x] [070] **The Pipe:** Implement an IPC Pipe (one writer, one reader).
- [x] [071] **The Arg:** Pass command line arguments (`argv`) to new processes.
- [x] [072] **The Clock:** Implement `sys_time()` and `sys_sleep()`.
- [x] [073] **The Lock:** Implement user-mode synchronization (Mutex/Futex).

---

## 🖱️ Rank X: The Human (Input & Events)

*Focus: You can't use a GUI with just a keyboard.*

- [ ] [074] **The Upgrade:** Rewrite Keyboard driver using `pc-keyboard` (State Machine).
- [ ] [075] **The Mouse:** Initialize PS/2 Mouse (enable IRQ12).
- [ ] [076] **The Packet:** Decode the 3-byte Mouse Packet (dx, dy, buttons).
- [ ] [077] **The Cursor:** Draw a software cursor sprite on the Framebuffer (XOR paint).
- [ ] [078] **The Event:** Create a kernel `EventBuffer` (Keyboard + Mouse events).
- [ ] [079] **The Feed:** Implement `sys_read` to let User apps read the EventBuffer.

## 🖼️ Rank XI: The Compositor (Display Server)

*Focus: The visual finish line. Drawing windows.*

- [ ] [080] **The Map:** Implement `sys_mmap` to share Framebuffer memory with User Mode.
- [ ] [081] **The Blit:** Write a fast memory copy routine (SIMD optimized) for rects.
- [ ] [082] **The Surface:** Define a `Window` struct (ID, x, y, width, height, buffer).
- [ ] [083] **The Server:** Create `user/display_server` process.
- [ ] [084] **The Client:** Create a shared memory protocol (Window creation requests).
- [ ] [085] **The Loop:** Render multiple windows back-to-front (Painter's Algorithm).
- [ ] [086] **The Clip:** Implement "Dirty Rectangles" (only redraw what changed).
- [ ] [087] **The Drag:** Bind Mouse Events to Window coordinates (move windows).
- [ ] [088] **The End:** Run `display_server` launching two windows: a Terminal and a Clock.

---

## 🔮 Rank XII: The Production (The Long Haul)

*Focus: Making it "Fail-Safe" and "General Purpose".*

- [ ] [089] **The Cores:** Enable SMP (Start up APIC on secondary CPUs).
- [ ] [090] **The Lock:** Rewrite kernel with Spinlocks/Mutexes for thread safety.
- [ ] [091] **The Drive:** Write an AHCI or NVMe driver (Real Persistence).
- [ ] [092] **The Write:** Implement FAT32 or Ext2 Write support.
- [ ] [093] **The Net:** Write an Intel E1000 or VirtIO-Net driver.
- [ ] [094] **The Stack:** Port a TCP/IP stack (smoltcp).
- [ ] [095] **The Lib:** Port a C Library (relibc or newlib).
- [ ] [096] **The Self:** Compile MinimalOS *inside* MinimalOS (Self-Hosting).
