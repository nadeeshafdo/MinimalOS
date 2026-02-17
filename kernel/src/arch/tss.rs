//! Task State Segment (TSS) for x86_64.
//!
//! The TSS holds the stack pointers for privilege level changes and
//! the Interrupt Stack Table (IST), used for guaranteed stack switches
//! during critical exceptions like Double Fault.

/// Size of each IST stack in bytes (16 KiB).
const IST_STACK_SIZE: usize = 4096 * 4;

/// Stack storage for IST entry 1 (used by Double Fault handler).
static mut DOUBLE_FAULT_STACK: [u8; IST_STACK_SIZE] = [0; IST_STACK_SIZE];

/// The 64-bit Task State Segment.
///
/// In long mode, the TSS does not store register state for task switching.
/// Instead it stores:
/// - RSP values for privilege level transitions (RSP0-RSP2)
/// - IST pointers for guaranteed stack switches on specific interrupts
/// - I/O permission bitmap offset
#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    /// Stack pointers for privilege level transitions.
    /// RSP0 is used when transitioning from Ring 3 to Ring 0.
    pub rsp: [u64; 3],
    reserved1: u64,
    /// Interrupt Stack Table (IST) entries.
    /// IST1-IST7 provide dedicated stacks for specific interrupt handlers.
    pub ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    /// Offset to the I/O permission bitmap from the TSS base.
    pub iomap_base: u16,
}

impl Tss {
    /// Create a new TSS with all fields zeroed.
    pub const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp: [0; 3],
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iomap_base: core::mem::size_of::<Self>() as u16,
        }
    }

    /// Initialize the TSS with IST stacks.
    ///
    /// Sets up IST1 with a dedicated stack for Double Fault handling.
    pub fn init(&mut self) {
        // IST1: Double Fault handler stack
        // Stack grows downward, so we set the pointer to the top of the allocation.
        let stack_top = unsafe {
            DOUBLE_FAULT_STACK.as_ptr().add(IST_STACK_SIZE) as u64
        };
        self.ist[0] = stack_top; // IST index 1 is stored at ist[0]
    }
}
