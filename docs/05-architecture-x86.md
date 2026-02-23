# x86_64 Architecture

## GDT (`kernel/src/arch/gdt.rs`)

### Segment Layout (syscall/sysret compatible)

| Index | Selector | Description | DPL |
|---|---|---|---|
| 0 | 0x00 | Null descriptor | — |
| 1 | 0x08 | Kernel Code (64-bit, readable) | 0 |
| 2 | 0x10 | Kernel Data (writable) | 0 |
| 3 | 0x18 | User Data (writable) | 3 |
| 4 | 0x20 | User Code (64-bit, readable) | 3 |
| 5–6 | 0x28 | TSS descriptor (128-bit, spans 2 entries) | 0 |

**Ordering matters for STAR MSR:**
- `syscall`: CS = STAR[47:32] = 0x08, SS = 0x08+8 = 0x10
- `sysret64`: CS = STAR[63:48]+16 = 0x10+16 = 0x20 (|3 by HW), SS = 0x10+8 = 0x18 (|3 by HW)

## TSS (`kernel/src/arch/tss.rs`)

```rust
#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    pub rsp: [u64; 3],      // RSP0 for Ring 3→0 transitions
    reserved1: u64,
    pub ist: [u64; 7],      // IST1-7 for dedicated interrupt stacks
    reserved2: u64,
    reserved3: u16,
    pub iomap_base: u16,
}
```

- **RSP0**: Kernel stack top — updated on every context switch via `Tss::set_rsp0()`
- **IST1**: Double fault handler stack (16 KiB)
- Per-core TSS instances in `CoreLocal` (avoids the Busy bit #GP)

## IDT (`kernel/src/arch/idt.rs`)

256-entry IDT with `IdtEntry` (128-bit each):
- `EntryOptions`: present, DPL, gate type (Interrupt/Trap), IST index
- Shared across all cores (IDT has no "Busy" bit)

### Registered Vectors

| Vector | Handler | Type | IST | Source |
|---|---|---|---|---|
| 3 | `breakpoint_handler` | Interrupt | — | INT3 |
| 8 | `double_fault_handler` | Interrupt | IST1 | CPU |
| 14 | `page_fault_handler` | Interrupt | — | CPU |
| 32 | `timer_handler` | Interrupt | — | APIC Timer |
| 33 | `keyboard_handler` | Interrupt | — | IRQ1 via I/O APIC |
| 44 | `mouse_handler` | Interrupt | — | IRQ12 via I/O APIC |
| 0xFF | `spurious_handler` | Interrupt | — | APIC Spurious |

## SMP (`kernel/src/arch/smp.rs`)

### CoreLocal Structure

```rust
#[repr(C)]
pub struct CoreLocal {
    pub core_id: u32,        // Offset 0 — read via gs:[0]
    pub apic_id: u32,
    tss: Tss,                // Per-core TSS
    gdt: Gdt,                // Per-core GDT
    selectors: Selectors,
    kernel_stack: [u8; 16K], // Ring 3→0 stack (RSP0)
    ist_stack: [u8; 16K],    // Double fault IST stack
}
```

- `MAX_CORES = 4`
- Static array: `CORE_LOCALS: [CoreLocal; 4]`
- `core_id()`: reads `gs:[0]` — zero-overhead per-core identification
- GS base set via `IA32_GS_BASE` and `IA32_KERNEL_GS_BASE` MSRs

### AP Boot Flow

1. BSP calls `wake_aps(smp_response)` with Limine SMP info
2. For each AP: init `CoreLocal`, set `goto_address` in Limine CPU struct
3. AP entry (`ap_entry`):
   - Load per-core GDT/TSS
   - Load shared IDT
   - Set GS base to CoreLocal
   - Disable legacy PIC (idempotent)
   - Init Local APIC (SVR, TPR)
   - **No timer enabled** (scheduler is single-core)
   - Signal ready via `AP_READY_COUNT`
   - Enter `sti; hlt` idle loop

### Current Limitation
AP timers are intentionally disabled. The global scheduler can't handle concurrent `do_schedule()` from multiple cores. APs idle until per-core runqueues are implemented.

## Syscall (`kernel/src/arch/syscall.rs`)

### MSR Configuration
- **EFER**: SCE bit enabled
- **STAR**: kernel base = 0x08, sysret base = 0x10
- **LSTAR**: points to `syscall_entry` naked function
- **SFMASK**: masks IF (interrupts disabled on entry)

### Entry Stub (naked asm)
```
syscall_entry:
  1. Save user RSP → SYSCALL_USER_RSP
  2. Load kernel RSP ← SYSCALL_KERNEL_RSP
  3. Push: RCX(user RIP), R11(user RFLAGS), callee-saved regs, user RSP
  4. Shuffle args into SysV ABI slots
  5. call syscall_dispatch
  6. Pop everything in reverse
  7. Restore user RSP
  8. sysretq
```

### Syscall Numbers
| Nr | Name | Args |
|---|---|---|
| 0 | SYS_LOG | ptr, len |
| 1 | SYS_EXIT | code |
| 22 | SYS_CAP_SEND | endpoint_handle, msg_ptr |
| 23 | SYS_CAP_RECV | msg_buf_ptr |

**Note**: Wasm actors use host functions directly, not syscalls. The syscall dispatcher exists for potential future Ring 3 actors.
