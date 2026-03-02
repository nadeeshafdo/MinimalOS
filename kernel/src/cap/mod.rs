// =============================================================================
// MinimalOS NextGen — Capability Subsystem
// =============================================================================
//
// Capabilities are unforgeable tokens that grant a thread the right to perform
// specific operations on specific kernel objects. This is the foundation of
// the security model — there are no ambient permissions, no UID checks,
// no ACLs. If you don't hold a capability, you can't access the object.
//
// Architecture:
//   - Each thread has a CNode (capability node) — a fixed-size array of slots.
//   - Slots are addressed by index (like file descriptors in POSIX).
//   - Each slot holds a Capability: a (object, rights) pair.
//   - Rights are bitmasks — you can restrict a capability by AND-ing rights.
//   - Capabilities can be transferred between threads via IPC.
//
// This module provides:
//   cnode.rs — CapRights, CapObject, Capability, CNode
// =============================================================================

pub mod cnode;
