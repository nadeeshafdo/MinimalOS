// =============================================================================
// MinimalOS NextGen — Architecture Abstraction
// =============================================================================
//
// This module re-exports the current architecture's HAL. Currently only
// x86_64 is supported. The rest of the kernel uses `crate::arch::*` and
// never directly references `x86_64`.
//
// To add a new architecture:
//   1. Create `arch/aarch64/mod.rs` with the same public interface
//   2. Add a `#[cfg(target_arch = "aarch64")]` here
//   3. Everything else just works
// =============================================================================

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
