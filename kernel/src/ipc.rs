//! IPC Subsystem — discrete message passing and capability routing.
//!
//! Messages are 48-byte cache-friendly packets that carry data *and*
//! capability grants simultaneously.  This is the communication backbone
//! for the microkernel — all actor interactions flow through IPC.
//!
//! **Security invariants:**
//! - Inline cap transfer checks `GRANT` permission on the source
//! - `cap_perms` mask enables explicit permission narrowing
//! - Cap insertion into target's table happens *before* message push
//!   (atomic: if CapTable full → syscall aborts, no message queued)
#![allow(dead_code)]

// ── Message ─────────────────────────────────────────────────────

/// A discrete message passed between actors.
///
/// 48 bytes — fits in a cache line.  The `cap_grant` field enables
/// zero-copy capability transfer: the kernel clones the sender's
/// capability into the receiver's CapTable during delivery, rewriting
/// `cap_grant` with the new composite handle.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Message {
	/// Method ID or message type (e.g. VFS_READ, UI_DRAW).
	pub label: u64,
	/// Raw data payload (24 bytes).
	pub data: [u64; 3],
	/// Composite handle of capability to transfer (0 = none).
	pub cap_grant: u64,
	/// Permission narrowing mask for the transfer.
	/// Granted permissions = `source.perms & cap_perms`.
	pub cap_perms: u32,
	/// Padding to keep struct at exactly 48 bytes, aligned.
	pub _pad: u32,
}

impl Message {
	/// Create an empty message.
	pub const fn empty() -> Self {
		Self {
			label: 0,
			data: [0; 3],
			cap_grant: 0,
			cap_perms: 0,
			_pad: 0,
		}
	}
}

// ── IPC Queue ───────────────────────────────────────────────────

/// Maximum messages in a per-process receive queue.
pub const IPC_QUEUE_SIZE: usize = 16;

/// Fixed-size ring buffer for incoming messages.
///
/// Non-blocking push (returns error if full), non-blocking pop
/// (returns `None` if empty).  Blocking semantics are handled
/// by the syscall layer via scheduler integration.
pub struct IpcQueue {
	messages: [Message; IPC_QUEUE_SIZE],
	head: usize,
	tail: usize,
	count: usize,
}

impl IpcQueue {
	/// Create a new empty queue.
	pub fn new() -> Self {
		Self {
			messages: [Message::empty(); IPC_QUEUE_SIZE],
			head: 0,
			tail: 0,
			count: 0,
		}
	}

	/// Push a message into the queue.
	///
	/// Returns `Err` if the queue is full.
	pub fn push(&mut self, msg: Message) -> Result<(), &'static str> {
		if self.count == IPC_QUEUE_SIZE {
			return Err("IPC queue full");
		}
		self.messages[self.tail] = msg;
		self.tail = (self.tail + 1) % IPC_QUEUE_SIZE;
		self.count += 1;
		Ok(())
	}

	/// Pop the oldest message from the queue.
	///
	/// Returns `None` if the queue is empty.
	pub fn pop(&mut self) -> Option<Message> {
		if self.count == 0 {
			return None;
		}
		let msg = self.messages[self.head];
		self.head = (self.head + 1) % IPC_QUEUE_SIZE;
		self.count -= 1;
		Some(msg)
	}

	/// Check if the queue is full.
	#[inline]
	pub fn is_full(&self) -> bool {
		self.count == IPC_QUEUE_SIZE
	}

	/// Check if the queue is empty.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.count == 0
	}
}
