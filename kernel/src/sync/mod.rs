// =============================================================================
// MinimalOS NextGen — Kernel Synchronization Primitives
// =============================================================================
//
// This module provides synchronization primitives for the kernel.
// In a kernel, we can't use std::sync (there is no std). We need our own
// primitives that work in a bare-metal, multi-core, interrupt-driven
// environment.
//
// IMPORTANT: Lock ordering rules (see architecture doc):
//   Level 1 (innermost): PMM bitmap lock
//   Level 2: Page table lock
//   Level 3: IPC endpoint locks
//   Level 4: Capability table lock
//   Level 5: Process table lock
//   Level 6 (outermost): Scheduler run queue lock
//
// NEVER acquire a lower-level lock while holding a higher-level lock.
// Violating this WILL cause deadlocks on multi-core.
// =============================================================================

pub mod spinlock;

