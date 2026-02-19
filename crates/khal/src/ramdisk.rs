//! RAMDisk block device driver.
//!
//! Wraps a contiguous region of memory (loaded by the bootloader as a module)
//! and exposes it as a read-only block device with 512-byte sectors.

/// Sector size in bytes.
pub const SECTOR_SIZE: usize = 512;

/// A read-only RAMDisk backed by a contiguous memory region.
pub struct RamDisk {
	base: *const u8,
	size: usize,
}

// The bootloader loads the module once and the pointer is stable for the
// lifetime of the kernel, so sharing across (the single) CPU is fine.
unsafe impl Send for RamDisk {}
unsafe impl Sync for RamDisk {}

impl RamDisk {
	/// Create a new `RamDisk` from a raw pointer and length.
	///
	/// # Safety
	/// `base` must point to a valid, readable memory region of at least `size`
	/// bytes that remains valid for the lifetime of this object.
	pub const unsafe fn new(base: *const u8, size: usize) -> Self {
		Self { base, size }
	}

	/// Total size of the ramdisk in bytes.
	#[inline]
	pub const fn size(&self) -> usize {
		self.size
	}

	/// Number of complete 512-byte sectors in the ramdisk.
	#[inline]
	pub const fn sector_count(&self) -> usize {
		self.size / SECTOR_SIZE
	}

	/// Base pointer of the ramdisk.
	#[inline]
	pub const fn base(&self) -> *const u8 {
		self.base
	}

	/// Read a raw byte slice of the full ramdisk contents.
	///
	/// # Safety
	/// Caller must ensure the backing memory is still valid.
	pub unsafe fn as_slice(&self) -> &[u8] {
		unsafe { core::slice::from_raw_parts(self.base, self.size) }
	}

	/// Read a single 512-byte sector by its LBA (Logical Block Address).
	///
	/// Returns `None` if the LBA is out of range.
	pub fn read_sector(&self, lba: usize) -> Option<&[u8; SECTOR_SIZE]> {
		let offset = lba * SECTOR_SIZE;
		if offset + SECTOR_SIZE > self.size {
			return None;
		}
		// SAFETY: bounds checked above; backing memory is valid.
		unsafe {
			let ptr = self.base.add(offset) as *const [u8; SECTOR_SIZE];
			Some(&*ptr)
		}
	}

	/// Read an arbitrary byte range from the ramdisk.
	///
	/// Returns `None` if the range extends beyond the ramdisk.
	pub fn read_bytes(&self, offset: usize, len: usize) -> Option<&[u8]> {
		if offset.checked_add(len)? > self.size {
			return None;
		}
		// SAFETY: bounds checked above.
		unsafe { Some(core::slice::from_raw_parts(self.base.add(offset), len)) }
	}
}
