// =============================================================================
// libmnos — Process & Memory Management Syscall Wrappers
// =============================================================================
//
// Safe wrappers around the Sprint 9 Phase 2 delegation syscalls:
//
//   SYS_SPAWN_PROCESS (6)  — Create a new empty process
//   SYS_ALLOC_MEMORY  (7)  — Allocate a physical frame via PmmAllocator cap
//   SYS_MAP_MEMORY    (8)  — Map a MemoryFrame into a process's address space
//   SYS_DELEGATE      (9)  — Copy a capability to a target process's CNode
//   SYS_SPAWN_THREAD  (10) — Create a Ring 3 thread in a target process
//
// These syscalls let the Init process (and any process with the right
// capabilities) create child processes, allocate/map memory, delegate
// capabilities, and spawn threads — all from Ring 3.
//
// =============================================================================

use crate::syscall::{SyscallError, syscall4};

/// Syscall numbers (must match kernel/src/arch/x86_64/syscall.rs).
const SYS_SPAWN_PROCESS: u64 = 6;
const SYS_ALLOC_MEMORY: u64 = 7;
const SYS_MAP_MEMORY: u64 = 8;
const SYS_DELEGATE: u64 = 9;
const SYS_SPAWN_THREAD: u64 = 10;

/// Creates a new process with an isolated PML4 and empty CNode.
///
/// The kernel allocates the process, registers it in the global process table,
/// and mints a `Process { pid }` capability into the first empty slot of the
/// caller's CNode.
///
/// # Returns
/// `Ok(slot)` — the CNode slot index where the new Process capability was placed.
/// `Err(SyscallError)` — CNode full or internal error.
#[inline(always)]
pub fn sys_spawn_process() -> Result<u64, SyscallError> {
    let result = unsafe { syscall4(SYS_SPAWN_PROCESS, 0, 0, 0, 0) };
    if result < u64::MAX - 10 {
        Ok(result)
    } else {
        Err(SyscallError(result))
    }
}

/// Allocates a zeroed physical frame from the PMM.
///
/// The caller must hold a `PmmAllocator` capability in `alloc_slot` with
/// WRITE permission. On success, a `MemoryFrame` capability is placed in
/// `target_slot` of the caller's CNode.
///
/// # Arguments
/// - `alloc_slot`:  CNode slot containing the PmmAllocator capability.
/// - `target_slot`: CNode slot where the new MemoryFrame cap will be placed.
///
/// # Returns
/// `Ok(())` on success, `Err(SyscallError)` on failure.
#[inline(always)]
pub fn sys_alloc_memory(alloc_slot: u64, target_slot: u64) -> Result<(), SyscallError> {
    let result = unsafe { syscall4(SYS_ALLOC_MEMORY, alloc_slot, target_slot, 0, 0) };
    if result == 0 {
        Ok(())
    } else {
        Err(SyscallError(result))
    }
}

/// Maps a physical frame into a process's address space.
///
/// The caller must hold BOTH a `Process` capability (in `proc_slot`) and a
/// `MemoryFrame` capability (in `frame_slot`).
///
/// # Arguments
/// - `proc_slot`:  CNode slot containing the target Process capability.
/// - `frame_slot`: CNode slot containing the MemoryFrame capability.
/// - `vaddr`:      Virtual address to map the frame at (must be page-aligned, lower half).
/// - `flags`:      Page table flags bitmask:
///                   bit 0 = WRITABLE
///                   bit 1 = EXECUTABLE (if clear → NO_EXECUTE)
///                 PRESENT and USER are always set by the kernel.
///
/// # Returns
/// `Ok(())` on success, `Err(SyscallError)` on failure.
#[inline(always)]
pub fn sys_map_memory(
    proc_slot: u64,
    frame_slot: u64,
    vaddr: u64,
    flags: u64,
) -> Result<(), SyscallError> {
    let result = unsafe { syscall4(SYS_MAP_MEMORY, proc_slot, frame_slot, vaddr, flags) };
    if result == 0 {
        Ok(())
    } else {
        Err(SyscallError(result))
    }
}

/// Copies a capability from the caller's CNode to a target process's CNode.
///
/// The caller must hold a `Process` capability in `proc_slot`.
///
/// # Arguments
/// - `proc_slot`: CNode slot containing the target Process capability.
/// - `src_slot`:  CNode slot in the caller's CNode to copy from.
/// - `dst_slot`:  CNode slot in the target process's CNode to copy into.
///
/// # Returns
/// `Ok(())` on success, `Err(SyscallError)` on failure.
#[inline(always)]
pub fn sys_delegate(
    proc_slot: u64,
    src_slot: u64,
    dst_slot: u64,
) -> Result<(), SyscallError> {
    let result = unsafe { syscall4(SYS_DELEGATE, proc_slot, src_slot, dst_slot, 0) };
    if result == 0 {
        Ok(())
    } else {
        Err(SyscallError(result))
    }
}

/// Creates a Ring 3 thread inside a target process.
///
/// The caller must hold a `Process` capability in `proc_slot`.
///
/// # Arguments
/// - `proc_slot`: CNode slot containing the target Process capability.
/// - `user_rip`:  Entry point (virtual address) for the new thread.
/// - `user_rsp`:  Initial stack pointer for the new thread.
///
/// # Returns
/// `Ok(tid)` — the thread ID of the newly spawned thread.
/// `Err(SyscallError)` on failure.
#[inline(always)]
pub fn sys_spawn_thread(
    proc_slot: u64,
    user_rip: u64,
    user_rsp: u64,
) -> Result<u64, SyscallError> {
    let result = unsafe { syscall4(SYS_SPAWN_THREAD, proc_slot, user_rip, user_rsp, 0) };
    if result < u64::MAX - 10 {
        Ok(result)
    } else {
        Err(SyscallError(result))
    }
}
