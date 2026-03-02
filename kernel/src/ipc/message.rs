// =============================================================================
// MinimalOS NextGen — IPC Message Format
// =============================================================================
//
// An IPC message is a small, fixed-size structure that threads exchange
// through Endpoints. Designed for register-passing (L4-style fastpath):
//   - 1 label (opcode / message type discriminant)
//   - 4 data registers (32 bytes of inline payload)
//   - 4 capability transfer slots (CNode indices)
//
// WHY FIXED SIZE:
//   No heap allocation per message. No fragmentation. The message lives
//   inside the Thread's TCB (ipc_buffer field), so send/recv is a
//   simple memcpy between two kernel-resident structs.
//
// WHY SMALL:
//   L4 microkernels discovered that most IPC messages are tiny (< 64 bytes).
//   Large data transfers are done via shared memory pages, not by copying
//   through the kernel. The IPC message carries the "control" information
//   (which pages, what operation), and the data lives in granted memory.
//
// CAPABILITY TRANSFER:
//   The `caps` array holds CNode slot indices from the SENDER's CNode.
//   During IPC, the kernel copies the referenced capabilities from the
//   sender's CNode slots into empty slots in the receiver's CNode.
//   This is the ONLY way capabilities propagate between threads.
//
// =============================================================================

/// Maximum inline data registers per message.
/// 4 × 8 bytes = 32 bytes — fits in 4 general-purpose registers.
pub const MSG_MAX_REGS: usize = 4;

/// Maximum capability transfers per message.
/// 4 caps is enough for typical microkernel operations (open file =
/// name server cap + memory cap + interrupt cap, with 1 spare).
pub const MSG_MAX_CAPS: usize = 4;

/// An IPC message: label + inline data + capability transfer slots.
///
/// This struct is embedded in each Thread's TCB as `ipc_buffer`.
/// Senders write their message here, and the kernel copies it
/// to the receiver's buffer during rendezvous.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IpcMessage {
    /// Caller-defined opcode / message type.
    /// The receiver uses this to dispatch to the correct handler.
    /// Convention: label 0 = empty/no-message.
    pub label: u64,

    /// Inline data registers. Interpreted by the receiver.
    /// For example, a "read file" message might use:
    ///   regs[0] = file offset
    ///   regs[1] = byte count
    ///   regs[2] = destination buffer address (in receiver's address space)
    pub regs: [u64; MSG_MAX_REGS],

    /// CNode slot indices (in the sender's CNode) of capabilities to transfer.
    /// Only the first `cap_count` entries are valid.
    pub caps: [u8; MSG_MAX_CAPS],

    /// Number of valid entries in `caps` (0..MSG_MAX_CAPS).
    pub cap_count: u8,
}

impl IpcMessage {
    /// An empty message — the default state of a thread's IPC buffer.
    pub const EMPTY: Self = Self {
        label: 0,
        regs: [0; MSG_MAX_REGS],
        caps: [0; MSG_MAX_CAPS],
        cap_count: 0,
    };

    /// Creates a new message with the given label and no data.
    pub const fn new(label: u64) -> Self {
        Self {
            label,
            regs: [0; MSG_MAX_REGS],
            caps: [0; MSG_MAX_CAPS],
            cap_count: 0,
        }
    }

    /// Creates a message with a label and inline data registers.
    pub const fn with_data(label: u64, regs: [u64; MSG_MAX_REGS]) -> Self {
        Self {
            label,
            regs,
            caps: [0; MSG_MAX_CAPS],
            cap_count: 0,
        }
    }

    /// Returns true if this is an empty (no-message) buffer.
    pub const fn is_empty(&self) -> bool {
        self.label == 0
    }
}
