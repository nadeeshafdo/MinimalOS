//! Capability Engine — unforgeable tokens for kernel object access.
//!
//! A capability is a kernel-managed (object, permissions) pair.
//! Processes can only access resources they hold capabilities for.
//!
//! **Security invariants:**
//! - No global identifiers — processes interact only via held capabilities.
//! - Handles are generation-guarded composites `(gen << 32 | index)` to
//!   prevent ABA handle-reuse vulnerabilities.
//! - GRANT bit is never auto-cleared — callers explicitly narrow permissions.
#![allow(dead_code)]

use alloc::string::String;

// ── Permission Bits ─────────────────────────────────────────────

/// Permission bitmask for capabilities.
pub mod perms {
	/// Can read from the object.
	pub const READ: u32 = 1 << 0;
	/// Can write to the object.
	pub const WRITE: u32 = 1 << 1;
	/// Can execute the object (reserved for Wasm modules).
	pub const EXEC: u32 = 1 << 2;
	/// Can transfer this capability to another actor via SYS_CAP_GRANT.
	pub const GRANT: u32 = 1 << 3;
	/// Can map the object into the actor's address space.
	pub const MAP: u32 = 1 << 4;
	/// All permissions.
	pub const ALL: u32 = READ | WRITE | EXEC | GRANT | MAP;
}

// ── Kernel Object Types ─────────────────────────────────────────

/// The kind of kernel object a capability refers to.
#[derive(Debug, Clone)]
pub enum ObjectKind {
	/// Slot is unused.
	Empty,
	/// A contiguous region of physical memory.
	Memory { phys: u64, pages: usize },
	/// A hardware interrupt line.
	IrqLine { irq: u8 },
	/// An x86 I/O port range.
	IoPort { base: u16, count: u16 },
	/// Kernel log (serial output).
	Log,
	/// IPC endpoint to another actor.
	/// `target_actor_id` is an internal kernel tracking ID, NOT a public PID.
	Endpoint { target_actor_id: u64 },
}

impl ObjectKind {
	/// Returns `true` if this slot is empty.
	#[inline]
	pub fn is_empty(&self) -> bool {
		matches!(self, ObjectKind::Empty)
	}
}

// ── Capability ──────────────────────────────────────────────────

/// A capability: unforgeable token = (kernel object + permissions).
///
/// The `generation` field prevents ABA handle-reuse attacks: when a
/// slot is freed and reused, its generation is incremented, invalidating
/// all handles that encode the old generation.
#[derive(Debug, Clone)]
pub struct Capability {
	pub object: ObjectKind,
	pub perms: u32,
	pub generation: u32,
}

impl Capability {
	/// Create an empty (unused) capability with a given generation.
	pub const fn empty(generation: u32) -> Self {
		Self {
			object: ObjectKind::Empty,
			perms: 0,
			generation,
		}
	}

	/// Check if this capability has the given permissions.
	#[inline]
	pub fn has_perms(&self, required: u32) -> bool {
		(self.perms & required) == required
	}
}

// ── Composite Handle ────────────────────────────────────────────

/// Pack a (generation, index) pair into a composite handle.
#[inline]
pub fn handle_pack(generation: u32, index: usize) -> u64 {
	((generation as u64) << 32) | (index as u64)
}

/// Unpack a composite handle into (generation, index).
#[inline]
pub fn handle_unpack(handle: u64) -> (u32, usize) {
	let generation = (handle >> 32) as u32;
	let index = (handle & 0xFFFF_FFFF) as usize;
	(generation, index)
}

// ── Capability Table ────────────────────────────────────────────

/// Maximum number of capabilities per actor.
pub const CAP_TABLE_SIZE: usize = 64;

/// Per-actor capability table.  Fixed-size array of capability slots.
///
/// Each slot tracks its own generation counter.  When a capability is
/// removed, the generation is incremented so stale handles are rejected.
pub struct CapTable {
	slots: [Capability; CAP_TABLE_SIZE],
}

impl CapTable {
	/// Create a new empty capability table.
	pub fn new() -> Self {
		// All slots start empty with generation 0.
		const EMPTY: Capability = Capability::empty(0);
		Self {
			slots: [EMPTY; CAP_TABLE_SIZE],
		}
	}

	/// Insert a capability into the first available empty slot.
	///
	/// The inserted capability's generation is set to the slot's current
	/// generation.  Returns the composite handle `(gen << 32 | index)`,
	/// or `None` if the table is full.
	pub fn insert(&mut self, object: ObjectKind, perms: u32) -> Option<u64> {
		for (i, slot) in self.slots.iter_mut().enumerate() {
			if slot.object.is_empty() {
				let gen = slot.generation;
				slot.object = object;
				slot.perms = perms;
				// generation stays at current value (already set)
				return Some(handle_pack(gen, i));
			}
		}
		None // table full
	}

	/// Insert a capability at a specific slot index (for init setup).
	///
	/// Returns the composite handle, or `None` if the slot is occupied.
	pub fn insert_at(&mut self, index: usize, object: ObjectKind, perms: u32) -> Option<u64> {
		if index >= CAP_TABLE_SIZE {
			return None;
		}
		let slot = &mut self.slots[index];
		if !slot.object.is_empty() {
			return None; // already occupied
		}
		let gen = slot.generation;
		slot.object = object;
		slot.perms = perms;
		Some(handle_pack(gen, index))
	}

	/// Look up a capability by composite handle.
	///
	/// Returns `None` if the handle is out of range, the generation
	/// doesn't match, or the slot is empty.
	pub fn get(&self, handle: u64) -> Option<&Capability> {
		let (gen, index) = handle_unpack(handle);
		if index >= CAP_TABLE_SIZE {
			return None;
		}
		let slot = &self.slots[index];
		if slot.generation != gen || slot.object.is_empty() {
			return None; // stale or empty
		}
		Some(slot)
	}

	/// Remove a capability by composite handle.
	///
	/// The slot's generation is incremented so all existing handles
	/// pointing to this slot become invalid.
	///
	/// Returns the removed capability, or `None` if the handle is invalid.
	pub fn remove(&mut self, handle: u64) -> Option<Capability> {
		let (gen, index) = handle_unpack(handle);
		if index >= CAP_TABLE_SIZE {
			return None;
		}
		let slot = &mut self.slots[index];
		if slot.generation != gen || slot.object.is_empty() {
			return None;
		}
		let removed = slot.clone();
		// Clear the slot and bump generation.
		slot.object = ObjectKind::Empty;
		slot.perms = 0;
		slot.generation = gen.wrapping_add(1);
		Some(removed)
	}

	/// Count the number of occupied (non-empty) slots.
	pub fn count(&self) -> usize {
		self.slots.iter().filter(|s| !s.object.is_empty()).count()
	}

	/// Debug: format a summary of the table contents.
	pub fn summary(&self) -> String {
		use alloc::format;
		let mut parts = alloc::vec::Vec::new();
		for (i, slot) in self.slots.iter().enumerate() {
			if slot.object.is_empty() {
				continue;
			}
			let kind = match &slot.object {
				ObjectKind::Empty => unreachable!(),
				ObjectKind::Memory { pages, .. } => format!("Mem({}pg)", pages),
				ObjectKind::IrqLine { irq } => format!("IRQ{}", irq),
				ObjectKind::IoPort { base, count } => format!("IO({:#x}+{})", base, count),
				ObjectKind::Log => "Log".into(),
				ObjectKind::Endpoint { target_actor_id } => format!("EP({})", target_actor_id),
			};
			let p = slot.perms;
			let flags = format!("{}{}{}{}{}",
				if p & perms::READ  != 0 { "R" } else { "-" },
				if p & perms::WRITE != 0 { "W" } else { "-" },
				if p & perms::EXEC  != 0 { "X" } else { "-" },
				if p & perms::GRANT != 0 { "G" } else { "-" },
				if p & perms::MAP   != 0 { "M" } else { "-" },
			);
			parts.push(format!("[{}]{}({})", i, kind, flags));
		}
		parts.join(" ")
	}
}
