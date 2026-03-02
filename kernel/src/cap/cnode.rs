// =============================================================================
// MinimalOS NextGen — Capability Node (CNode)
// =============================================================================
//
// A CNode is a thread's capability table — a fixed-size array of capability
// slots. Each slot holds a Capability that grants specific rights over a
// specific kernel object.
//
// DESIGN DECISIONS:
//   - Fixed 64 slots per CNode: avoids heap allocation, embedded in Thread TCB.
//     64 slots × ~32 bytes = 2 KiB — fits comfortably alongside the TCB.
//   - Rights are a bitmask (not enum): allows bitwise AND for restriction.
//     You can derive a weaker capability by masking off rights.
//   - CapObject::Empty replaces Option<>: keeps the struct trivially copyable
//     and avoids niche optimization surprises in repr(C) structs.
//   - No global capability registry: capabilities live exclusively in CNodes.
//     To access an object, you must hold a capability in YOUR CNode. Period.
//
// SECURITY MODEL:
//   - Capabilities are unforgeable: only the kernel can create them.
//   - Capabilities are transferable: threads can grant caps to other threads
//     via IPC message cap_slots (only if they hold GRANT right).
//   - Capabilities are restrictable: you can derive weaker caps (fewer rights)
//     but never escalate.
//   - Revocation: delete a slot → that thread loses access immediately.
//     (Future: full revocation trees for cascading delete.)
//
// =============================================================================

/// Number of capability slots per CNode.
/// 64 is enough for early bring-up. Can increase later if needed.
pub const CNODE_SLOTS: usize = 64;

// =============================================================================
// Capability Rights
// =============================================================================

/// Rights bitmask — determines what operations a capability permits.
///
/// Rights are combined with bitwise OR and restricted with bitwise AND.
/// A derived capability can never have MORE rights than its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct CapRights(u8);

impl CapRights {
    /// No rights — the capability exists but grants nothing.
    pub const NONE: Self = Self(0);

    /// Permission to read/receive from the referenced object.
    pub const READ: Self = Self(0x01);

    /// Permission to write/send to the referenced object.
    pub const WRITE: Self = Self(0x02);

    /// Permission to execute (map executable pages, invoke endpoints).
    pub const EXEC: Self = Self(0x04);

    /// Permission to transfer (grant) this capability to another thread via IPC.
    pub const GRANT: Self = Self(0x08);

    /// Permission to revoke derived capabilities (future use).
    pub const REVOKE: Self = Self(0x10);

    /// Full rights — used when the kernel creates an initial capability.
    pub const ALL: Self = Self(0x01 | 0x02 | 0x04 | 0x08 | 0x10);

    /// Creates a new rights bitmask from a raw byte.
    pub const fn from_raw(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Checks if this rights mask contains the specified right.
    pub const fn contains(self, other: CapRights) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Restricts rights by AND-ing: result has only the intersection.
    /// Used when deriving a weaker capability.
    pub const fn restrict(self, mask: CapRights) -> CapRights {
        CapRights(self.0 & mask.0)
    }
}

// =============================================================================
// Capability Object Reference
// =============================================================================

/// The kernel object a capability refers to.
///
/// Each variant identifies a different type of kernel-managed resource.
/// The discriminant is used to determine what operations are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapObject {
    /// Slot is empty — no capability stored here.
    Empty,

    /// IPC endpoint — a rendezvous point for synchronous message passing.
    /// The `id` uniquely identifies the endpoint in the kernel's table.
    Endpoint { id: u64 },

    /// Physical memory frame(s) — grants access to physical page(s).
    /// `phys` is the base physical address (page-aligned).
    /// `order` is the allocation order (0 = 4KiB, 1 = 8KiB, etc.).
    MemoryFrame { phys: u64, order: u8 },

    /// Hardware interrupt line — grants the right to receive IRQ notifications.
    /// `irq` is the global system interrupt (GSI) number.
    Interrupt { irq: u32 },

    /// Thread control — grants control over another thread (suspend/resume/kill).
    /// `tid` is the target thread's ID.
    ThreadControl { tid: u64 },
}

// =============================================================================
// Capability
// =============================================================================

/// A single capability slot: object reference + rights mask.
///
/// This is the fundamental unit of the security model. A thread can only
/// interact with a kernel object if it holds a Capability for that object
/// in its CNode, AND the Capability's rights permit the operation.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    /// Which kernel object this capability refers to.
    pub object: CapObject,

    /// What operations are permitted on that object.
    pub rights: CapRights,
}

impl Capability {
    /// An empty capability — the default state of all CNode slots.
    pub const EMPTY: Self = Self {
        object: CapObject::Empty,
        rights: CapRights::NONE,
    };

    /// Creates a new capability with the given object and rights.
    pub const fn new(object: CapObject, rights: CapRights) -> Self {
        Self { object, rights }
    }

    /// Returns true if this slot is empty (no capability).
    pub const fn is_empty(&self) -> bool {
        matches!(self.object, CapObject::Empty)
    }
}

// =============================================================================
// CNode — Per-Thread Capability Table
// =============================================================================

/// Per-thread capability table — a fixed-size array of capability slots.
///
/// Threads reference capabilities by slot index (0..CNODE_SLOTS-1), similar
/// to how POSIX processes reference files by file descriptor number.
///
/// Embedded directly in the Thread struct (no separate heap allocation).
pub struct CNode {
    /// The capability slots.
    pub slots: [Capability; CNODE_SLOTS],
}

impl CNode {
    /// Creates a new CNode with all slots empty.
    pub const fn new() -> Self {
        Self {
            slots: [Capability::EMPTY; CNODE_SLOTS],
        }
    }

    /// Looks up a capability by slot index.
    /// Returns None if the index is out of bounds or the slot is empty.
    pub fn lookup(&self, index: usize) -> Option<&Capability> {
        if index >= CNODE_SLOTS {
            return None;
        }
        let cap = &self.slots[index];
        if cap.is_empty() {
            None
        } else {
            Some(cap)
        }
    }

    /// Inserts a capability into the first empty slot.
    /// Returns the slot index on success, or None if the CNode is full.
    pub fn insert(&mut self, cap: Capability) -> Option<usize> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_empty() {
                *slot = cap;
                return Some(i);
            }
        }
        None
    }

    /// Inserts a capability at a specific slot index.
    /// Fails if the index is out of bounds or the slot is already occupied.
    pub fn insert_at(&mut self, index: usize, cap: Capability) -> Result<(), ()> {
        if index >= CNODE_SLOTS {
            return Err(());
        }
        if !self.slots[index].is_empty() {
            return Err(());
        }
        self.slots[index] = cap;
        Ok(())
    }

    /// Removes a capability from the specified slot.
    /// Returns the removed capability, or None if the slot was empty.
    pub fn remove(&mut self, index: usize) -> Option<Capability> {
        if index >= CNODE_SLOTS {
            return None;
        }
        let cap = self.slots[index];
        if cap.is_empty() {
            return None;
        }
        self.slots[index] = Capability::EMPTY;
        Some(cap)
    }

    /// Returns the number of occupied (non-empty) slots.
    pub fn count(&self) -> usize {
        self.slots.iter().filter(|c| !c.is_empty()).count()
    }
}
