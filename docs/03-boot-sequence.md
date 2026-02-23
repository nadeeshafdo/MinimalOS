# Kernel Boot Sequence

The kernel entry point is `_start()` in `kernel/src/main.rs`. The boot proceeds through numbered "quests" that were developed incrementally.

## Boot Steps (in order)

### 1. Serial Logging
```
klog::init() → COM1 UART 16550 at 115200 baud
```

### 2. Limine Validation
Checks `BASE_REVISION.is_supported()` — halts if unsupported.

### 3. [022] Disable Legacy PIC
```
khal::pic::disable()
```
Remaps 8259 PIC to vectors 32–47, masks all IRQs. Required before APIC.

### 4. [019] Load IDT
```
traps::init_idt()
```
- Creates BSP's per-core GDT/TSS via `smp::init_bsp(0)`
- Stores BSP TSS pointer for context-switch RSP0 updates
- Builds shared IDT with all handlers (breakpoint, double fault, page fault, timer, keyboard, mouse, spurious)
- Loads IDT on BSP

### 5. [046] Enable syscall/sysret
```
arch::syscall::init(bsp_core_local().kernel_rsp0())
```
Configures EFER.SCE, STAR, LSTAR, SFMASK MSRs. Entry stub in naked asm.

### 6. [023] Enable APIC
- Read APIC base from IA32_APIC_BASE MSR
- Map APIC MMIO via HHDM (2 MiB huge page)
- `khal::apic::init(hhdm_offset)` → enable SVR, set TPR=0
- Map and init I/O APIC at `0xFEC0_0000`

### 7. [027–030] Physical Memory Manager
```
memory::census()              → log memory map entries
memory::pmm::init()           → bitmap allocator from USABLE regions
pmm::alloc_frame() / free()   → test alloc/free cycle
```

### 8. [031–033] Paging
```
memory::paging::init(hhdm_offset)
map_page(test_virt, test_phys, KERNEL_RW)
translate(test_virt) → verify roundtrip
```

### 9. [034–036] Kernel Heap
```
memory::heap::init()          → 64 KiB initial, linked_list_allocator
Box::new(42), Vec::push()     → verify dynamic allocation
```

### 10. Shared User Region
```
memory::paging::init_shared_user_region()
```
Pre-allocates PML4[384] → PDPT so all processes share window buffer mappings.

### 11. [024] APIC Timer
```
khal::apic::enable_timer(vector=32, count=0x200000, div=By16)
sti → enable interrupts
```
Periodic mode — drives preemptive scheduling.

### 12. [020] Test Breakpoint
```
int3 → breakpoint handler fires → confirms IDT works
```

### 13. Framebuffer
Stores `FbInfo` (phys_addr, width, height, pitch, bpp) for capability-based access by UI actors. No direct rendering by the kernel.

### 14. [038–041] PS/2 Input
- Init keyboard state machine (`pc-keyboard` crate)
- Init PS/2 mouse (enable aux port, set defaults, enable reporting)
- Drain stale bytes, then enable IRQ1 (keyboard) and IRQ12 (mouse)

### 15. [089] Wake APs (SMP)
```
arch::smp::wake_aps(smp_response)
```
Each AP gets: CoreLocal init → GDT/TSS → shared IDT → GS base → APIC init → idle loop (`sti; hlt`). **No timer on APs** — scheduler is single-core.

### 16. [090] Per-Core Arenas
```
memory::pmm::activate_caches()     → per-core frame caches
memory::heap::init_arenas()        → 2 MiB linked_list_allocator per core
```

### 17. [053] RAMDisk Detection
```
MODULE_REQUEST → ramdisk.tar module
wasm::init_ramdisk(base, size)     → store globally
```

### 18. Create Idle Process
```
Process::new("idle", cr3, 0, 0)
idle.state = Running
SCHEDULER.set_current(idle)
```

### 19. CLI — Critical Section Start
Disable interrupts before spawning actors to prevent preemption during capability wiring.

### 20. Spawn Wasm Actors
```
wasm::spawn_wasm("vfs.wasm", |caps| { ... })       → PID 1
wasm::spawn_wasm("ui_server.wasm", |caps| { ... })  → PID 2
wasm::spawn_wasm("shell.wasm", |caps| { ... })      → PID 3
```

### 21. Post-Spawn Endpoint Wiring
Under scheduler lock, inject Endpoint capabilities into each actor's CapTable (see `09-capability-map.md`).

### 22. [064] First Schedule
```
task::process::do_schedule()
```
Context-switches from idle into the first ready actor.
