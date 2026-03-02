// =============================================================================
// MinimalOS NextGen — Inter-Process Communication (IPC) Subsystem
// =============================================================================
//
// L4-style synchronous IPC: the sender blocks until a receiver is ready
// (and vice versa). Messages are small, fixed-size, register-passable.
// Capabilities can be transferred alongside data.
//
// Architecture:
//   - Endpoints are rendezvous points (not mailboxes or channels).
//   - send() blocks until a matching recv(), and vice versa.
//   - On rendezvous, the message is copied directly between thread buffers
//     (zero-copy from the kernel's perspective — no intermediate buffer).
//   - Fastpath: if the partner is already waiting, the copy + wakeup
//     happens immediately without blocking the caller.
//
// SMP Safety (the critical part):
//   - Endpoint queues hold Box<Thread> — full ownership transfer.
//   - Interrupts are disabled BEFORE locking the endpoint to prevent
//     lost-wakeup races (see architecture doc for full analysis).
//   - schedule() knows not to requeue BlockedSend/BlockedRecv threads.
//
// This module provides:
//   message.rs  — IpcMessage format
//   endpoint.rs — Endpoint with send/recv
// =============================================================================

pub mod message;
pub mod endpoint;
