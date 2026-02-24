// =============================================================================
// MinimalOS NextGen — Kernel Utilities
// =============================================================================
//
// Shared utilities used across the entire kernel.
// These are deliberately minimal — just the essentials.
//
//   logger.rs — kprint!/kprintln! macros (serial + framebuffer output)
//   panic.rs  — panic handler (what happens when the kernel panics)
// =============================================================================

pub mod logger;
pub mod panic;
