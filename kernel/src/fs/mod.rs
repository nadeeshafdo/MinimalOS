// =============================================================================
// MinimalOS NextGen — Filesystem Subsystem
// =============================================================================
//
// Contains parsers for structured file formats needed during boot:
//   - tar: Read-only USTAR archive parser for the initrd
//   - elf: ELF64 executable loader for spawning user processes
//
// These are NOT full filesystem drivers. They are minimal, zero-alloc parsers
// that operate on byte slices already loaded into memory by the bootloader.
// =============================================================================

pub mod tar;
pub mod elf;
