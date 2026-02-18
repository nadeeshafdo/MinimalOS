//! USTAR tar archive parser.
//!
//! Parses a POSIX.1-2001 (USTAR) tar archive that has been loaded into memory
//! (e.g. from a RAMDisk module).  Read-only; sufficient for listing and
//! reading files from a boot-time ramdisk.

use khal::ramdisk::RamDisk;

/// Size of a single tar block (header or data padding unit).
const BLOCK: usize = 512;

/// Offset and size of the `magic` field in a USTAR header.
const MAGIC_OFFSET: usize = 257;
const MAGIC_LEN: usize = 5; // "ustar" (without trailing NUL variant byte)

/// A parsed TAR entry header.
#[derive(Debug)]
pub struct TarEntry<'a> {
    /// File name (NUL-terminated in the archive, trimmed here).
    pub name: &'a str,
    /// File size in bytes (decoded from the octal `size` field).
    pub size: usize,
    /// Type flag character (e.g. `'0'` = regular file, `'5'` = directory).
    pub typeflag: u8,
    /// Byte slice of the file contents (may be empty for directories).
    pub data: &'a [u8],
}

/// Iterator over the entries of a USTAR tar archive stored in a `RamDisk`.
pub struct TarIter<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> TarIter<'a> {
    /// Create a new TAR iterator from a `RamDisk`.
    ///
    /// # Safety
    /// The `RamDisk`'s backing memory must be valid for the returned lifetime.
    pub unsafe fn new(disk: &'a RamDisk) -> Self {
        Self {
            buf: unsafe { disk.as_slice() },
            offset: 0,
        }
    }

    /// Create a TAR iterator directly from a byte slice.
    #[allow(dead_code)]
    pub fn from_bytes(buf: &'a [u8]) -> Self {
        Self { buf, offset: 0 }
    }
}

impl<'a> Iterator for TarIter<'a> {
    type Item = TarEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Need at least one full header block.
            if self.offset + BLOCK > self.buf.len() {
                return None;
            }

            let header = &self.buf[self.offset..self.offset + BLOCK];

            // Two consecutive zero blocks mark the end of archive.
            if header.iter().all(|&b| b == 0) {
                return None;
            }

            // Validate USTAR magic ("ustar").
            let magic = &header[MAGIC_OFFSET..MAGIC_OFFSET + MAGIC_LEN];
            if magic != b"ustar" {
                // Not a valid header — skip this block and try the next.
                self.offset += BLOCK;
                continue;
            }

            // ── Parse name (bytes 0..100) ───────────────────────
            let name_bytes = &header[0..100];
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(100);
            let name = core::str::from_utf8(&name_bytes[..name_end]).unwrap_or("<invalid>");

            // ── Parse size (bytes 124..136, octal ASCII) ────────
            let size = parse_octal(&header[124..136]);

            // ── Type flag (byte 156) ────────────────────────────
            let typeflag = header[156];

            // Data immediately follows the header, rounded up to BLOCK.
            let data_start = self.offset + BLOCK;
            let data_end = data_start + size;

            let data = if size > 0 && data_end <= self.buf.len() {
                &self.buf[data_start..data_end]
            } else {
                &[]
            };

            // Advance past header + data (data padded to BLOCK boundary).
            let data_blocks = (size + BLOCK - 1) / BLOCK;
            self.offset += BLOCK + data_blocks * BLOCK;

            return Some(TarEntry {
                name,
                size,
                typeflag,
                data,
            });
        }
    }
}

/// Parse an octal ASCII string (with possible NUL/space padding) into `usize`.
fn parse_octal(field: &[u8]) -> usize {
    let mut value: usize = 0;
    for &b in field {
        if b == 0 || b == b' ' {
            break;
        }
        if b >= b'0' && b <= b'7' {
            value = value * 8 + (b - b'0') as usize;
        }
    }
    value
}

/// Find a file by name in the tar archive and return its entry.
pub fn find_file<'a>(disk: &'a RamDisk, name: &str) -> Option<TarEntry<'a>> {
    let iter = unsafe { TarIter::new(disk) };
    for entry in iter {
        // Strip leading "./" prefix that `tar` adds.
        let entry_name = entry.name.strip_prefix("./").unwrap_or(entry.name);
        let search_name = name.strip_prefix("./").unwrap_or(name);
        if entry_name == search_name {
            return Some(entry);
        }
    }
    None
}
