// =============================================================================
// MinimalOS NextGen — Kernel Drivers
// =============================================================================
//
// IMPORTANT DESIGN NOTE:
//   These are the ONLY drivers that live in kernel space. They exist here
//   because they're needed before userspace is running:
//
//   framebuffer.rs — Text rendering to the UEFI framebuffer (boot messages)
//   timer.rs       — LAPIC timer for scheduler preemption (Sprint 3)
//
//   ALL OTHER DRIVERS run in userspace as normal processes with
//   IoPort/DeviceMmio capabilities. This includes:
//     - Disk drivers (AHCI, NVMe)
//     - Network drivers (WiFi, Ethernet)
//     - USB drivers
//     - GPU drivers (beyond basic framebuffer)
//     - Audio drivers
//     - Input device drivers (keyboard, mouse, touchpad)
//
//   The kernel doesn't know or care about these devices. It just provides
//   the capability tokens that let userspace drivers access the hardware.
// =============================================================================

pub mod framebuffer;
