//! Global RAMDisk storage.
//!
//! The Limine bootloader provides the ramdisk module pointer at boot.
//! We store it here so that `sys_spawn` can find ELF files later.

use khal::ramdisk::RamDisk;
use spin::Once;

/// The global ramdisk instance, initialised once during boot.
static RAMDISK: Once<RamDisk> = Once::new();

/// Store the ramdisk globally.
///
/// # Safety
/// The `base` pointer must remain valid for the kernel's lifetime.
pub unsafe fn init(base: *const u8, size: usize) {
	RAMDISK.call_once(|| unsafe { RamDisk::new(base, size) });
	klog::debug!("Global ramdisk stored ({} bytes)", size);
}

/// Get a reference to the global ramdisk.
///
/// Returns `None` if `init()` has not been called yet.
pub fn get() -> Option<&'static RamDisk> {
	RAMDISK.get()
}
