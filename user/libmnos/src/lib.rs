// =============================================================================
// libmnos — MinimalOS Userspace Library
// =============================================================================
//
// This is the userspace-side of the MinimalOS syscall ABI.
//
// Ring 3 code cannot execute privileged instructions (IN/OUT, HLT, MOV CR3,
// etc.). The ONLY way to interact with the kernel or hardware is via SYSCALL.
// This library provides safe, ergonomic Rust wrappers around the raw inline
// assembly syscall instruction.
//
// DESIGN PRINCIPLES:
//   - Zero dependencies beyond `core` — no allocator needed.
//   - Each syscall wrapper is a thin inline function → zero overhead.
//   - Error codes are returned as Result types for Rust ergonomics.
//   - All capability slot references are explicit (no global state).
//
// SYSCALL ABI (matches kernel/src/arch/x86_64/syscall.rs):
//   RAX = syscall number
//   RDI = arg0 (typically CNode slot index)
//   RSI = arg1
//   RDX = arg2
//   R10 = arg3
//   Return: RAX = result (0 = success)
//   For SYS_RECV: RDI = label, RSI = data[0], RDX = data[1], R10 = data[2]
//   For SYS_PORT_IN: RDI = byte value
//   CPU-clobbered: RCX (user RIP), R11 (user RFLAGS)
//
// =============================================================================

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod syscall;
pub mod ipc;
pub mod io;
pub mod irq;
pub mod process;
pub mod heap;

use linked_list_allocator::LockedHeap;

/// Ring 3 global allocator — fed by `init_heap()` at startup.
#[global_allocator]
pub static HEAP: LockedHeap = LockedHeap::empty();

/// Out-of-memory handler for the `alloc` crate.
#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Ring 3 heap OOM: {:?}", layout);
}
