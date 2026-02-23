# Memory Subsystem

## Address Space Layout

| Virtual Range | Usage |
|---|---|
| `0x0000_0000` – `0x0000_8000_0000_0000` | User space (unused in SASOS) |
| HHDM (varies, ~`0xFFFF_8000_0000_0000`) | Physical memory direct map (set by Limine) |
| `0xFFFF_A000_0000_0000` | Global kernel heap (up to 16 MiB) |
| `0xFFFF_A000_1000_0000` | Per-core arenas (4 × 2 MiB = 8 MiB) |
| `0xFFFF_C000_0000_0000` | Shared user region (PML4[384], window buffers) |
| `0xFFFF_FFFF_8000_0000` | Kernel code/data (higher half, linker script) |

## Physical Memory Manager (`kernel/src/memory/pmm.rs`)

### Design
- **Bitmap allocator**: 1 bit per 4 KiB frame. Bit 1 = used, 0 = free
- Bitmap carved from the first USABLE region large enough
- Frame 0 permanently marked used (null-page protection)
- Search hint avoids re-scanning from the start

### Per-Core Frame Caches (SMP Optimization)
- Each core has a local cache of 32 pre-allocated frames
- `alloc_frame()` fast path: pop from cache (zero locking)
- `alloc_frame()` slow path: lock global bitmap, refill cache (16 frames)
- `free_frame()` fast path: push to cache
- `free_frame()` slow path: if cache full, drain half back to bitmap
- Activated after SMP init via `activate_caches()`

### API
```rust
pub fn init(hhdm_offset: u64, entries: &[&Entry])   // One-time init
pub fn alloc_frame() -> Option<u64>                   // Returns phys addr
pub fn free_frame(phys_addr: u64)                     // Panics on double-free
pub fn free_frame_count() -> usize                    // Approximate count
pub fn activate_caches()                              // Enable per-core caches
```

## Paging (`kernel/src/memory/paging.rs`)

### Design
- 4-level x86_64 page tables (PML4 → PDPT → PD → PT)
- Intermediate tables allocated on demand from PMM
- HHDM used to access physical page-table frames
- Supports 4 KiB, 2 MiB huge, and 1 GiB huge pages in translation

### API
```rust
pub fn init(hhdm_offset: u64)
pub fn hhdm_offset() -> u64
pub fn map_page(virt: u64, phys: u64, flags: PageFlags)          // Current CR3
pub fn translate(virt: u64) -> Option<u64>                        // Current CR3
pub fn map_page_in(pml4_phys, virt, phys, flags)                  // Specific PML4
pub fn translate_in(pml4_phys, virt) -> Option<u64>               // Specific PML4
pub fn create_user_page_table() -> Option<u64>                    // New PML4, copies upper half
pub fn free_user_page_table(pml4_phys: u64)                       // Recursive free lower half
pub fn init_shared_user_region()                                   // PML4[384] pre-alloc
```

### PageFlags
```rust
PRESENT, WRITABLE, USER, WRITE_THROUGH, CACHE_DISABLE, HUGE, NO_EXECUTE
KERNEL_RW = PRESENT | WRITABLE
USER_RW   = PRESENT | WRITABLE | USER
```

## Kernel Heap (`kernel/src/memory/heap.rs`)

### Architecture
- **Global heap**: `linked_list_allocator::Heap` at `0xFFFF_A000_0000_0000`
  - Grows on demand from PMM (up to 16 MiB)
  - Protected by `spin::Mutex`
  - Initial size: 64 KiB (16 pages)
- **Per-core arenas**: 2 MiB each at `0xFFFF_A000_1000_0000 + core_id * 2MiB`
  - Serves allocations ≤ 4 KiB
  - Uses `RefCell<Heap>` — safe because each core only touches its own + interrupts disabled
  - Falls back to global heap if arena is exhausted
- **Deallocation routing**: checks if pointer falls in any arena's address range; if not, routes to global heap
- **Interrupt safety**: `save_and_disable_interrupts()` / `restore_interrupts()` wraps every alloc/dealloc

### Allocation Flow
```
alloc(layout):
  1. Disable interrupts
  2. If ARENAS_ACTIVE && size ≤ 4096:
     a. Try CORE_ARENAS[core_id].alloc() → return if non-null
  3. Lock global heap
  4. Try allocate_first_fit()
  5. If fail: grow heap from PMM, extend(), retry
  6. Restore interrupts

dealloc(ptr, layout):
  1. Disable interrupts
  2. If ARENAS_ACTIVE:
     a. Check each arena's [base, end) range
     b. If match → arena.dealloc() → return
  3. Lock global heap → deallocate()
  4. Restore interrupts
```

## APIC MMIO Mapping (`kernel/src/memory/mod.rs`)

The `map_apic_mmio()` function creates 2 MiB huge page mappings in the HHDM for:
- Local APIC at `0xFEE0_0000`
- I/O APIC at `0xFEC0_0000`

Uses a static `APIC_PD_PAGE` when the PDPT entry doesn't exist yet. Includes a page-table walk (`virt_to_phys`) for converting kernel BSS virtual addresses to physical.
