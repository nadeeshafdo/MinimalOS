# MinimalOS Quest Tracker

> **Versioning:** `v0.0.{AchievementCount}` — current: **`v0.0.28`**
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

## ⚡ Rank III: The Reflexes (Interrupts & CPU)

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

## 🧠 Rank IV: The Mind (Memory Management)

*Focus: Moving from raw addresses to managed memory.*

- [x] [027] **The Census:** Iterate over the Limine Memory Map and calculate total RAM in bytes.
- [ ] [028] **The Accountant:** Implement a Bitmap Allocator (Physical Memory Manager).
- [ ] [029] **Mine!:** Successfully call `pmm_alloc_frame()` and get a valid address.
- [ ] [030] **Return It:** Successfully call `pmm_free_frame()` and update the bitmap.
- [ ] [031] **Higher Plane:** Create the recursive Page Table structure.
- [ ] [032] **The Mapper:** Write `map_page(virt, phys, flags)`.
- [ ] [033] **The Translator:** Write `virt_to_phys(addr)`.
- [ ] [034] **The Heap:** Initialize the `GlobalAlloc` trait.
- [ ] [035] **Dynamic Power:** Successfully use `Box::new(10)` inside the kernel.
- [ ] [036] **Vectorization:** Successfully push an item to a `Vec<i32>`.

## ⌨️ Rank V: The Senses (Input & Drivers) 🟡

*Focus: Interacting with the user.*

- [x] [037] **Port IO:** Implement `inb` and `outb` wrappers for x86 port communication.
- [ ] [038] **PS/2 Controller:** Read the status register of the PS/2 port.
- [ ] [039] **Key Down:** Receive a raw scancode when a key is pressed.
- [ ] [040] **Decoder:** Create a match statement to convert Scancodes to ASCII characters.
- [ ] [041] **The Echo:** Type on the keyboard and see letters appear on the MinimalOS screen.
- [ ] [042] **Backspace:** Implement backspace logic in the `kprint!` buffer.
- [x] [043] **Serial Killer:** Implement a driver for the COM1 Serial Port (logging to host).
