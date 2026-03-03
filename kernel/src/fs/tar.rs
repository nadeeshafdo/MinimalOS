// =============================================================================
// MinimalOS NextGen — USTAR TAR Archive Parser (Read-Only)
// =============================================================================
//
// Parses a USTAR-format TAR archive loaded into memory by the Limine
// bootloader as a boot module. The kernel uses this to locate executables
// (Init, serial_drv, etc.) in the initrd without needing a real filesystem.
//
// TAR FORMAT:
//   A TAR archive is a sequence of 512-byte blocks:
//     [header block][data blocks...][header block][data blocks...]...[zero blocks]
//
//   Each file entry consists of:
//     1. A 512-byte header containing filename, size (octal ASCII), type, etc.
//     2. ceil(size / 512) * 512 bytes of file data (padded to 512-byte boundary)
//
//   The archive ends with two consecutive all-zero 512-byte blocks.
//
// USTAR MAGIC:
//   Bytes 257-262 of the header contain "ustar\0" (or "ustar ") to identify
//   the USTAR format. We accept both variants for robustness.
//
// LIMITATIONS:
//   - Read-only: no modification, no extraction to separate buffers
//   - Returns slices into the original archive memory — zero-copy
//   - No support for long filenames (> 100 + 155 prefix chars)
//   - No support for sparse files, extended headers, or GNU extensions
//
// This is sufficient for an initrd containing a handful of ELF binaries.
// =============================================================================

use core::str;

/// Size of a TAR block (header and data alignment).
const BLOCK_SIZE: usize = 512;

// =============================================================================
// TAR Header (USTAR format)
// =============================================================================

/// Raw USTAR header — 512 bytes, directly overlaid on the archive memory.
///
/// Field sizes match the POSIX.1-2001 USTAR specification exactly.
/// All numeric fields are stored as ASCII octal strings (null-terminated).
#[repr(C, packed)]
struct TarHeader {
    /// Filename (null-terminated, up to 100 chars).
    name: [u8; 100],
    /// File mode (octal ASCII).
    mode: [u8; 8],
    /// Owner UID (octal ASCII).
    uid: [u8; 8],
    /// Owner GID (octal ASCII).
    gid: [u8; 8],
    /// File size in bytes (octal ASCII, up to 11 digits + null).
    size: [u8; 12],
    /// Last modification time (UNIX timestamp, octal ASCII).
    mtime: [u8; 12],
    /// Header checksum (octal ASCII).
    checksum: [u8; 8],
    /// File type flag (single ASCII character).
    /// '0' or '\0' = regular file, '5' = directory, etc.
    typeflag: u8,
    /// Name of linked file (for hard/soft links).
    linkname: [u8; 100],
    /// USTAR magic: "ustar\0" (POSIX) or "ustar " (GNU).
    magic: [u8; 6],
    /// USTAR version: "00".
    version: [u8; 2],
    /// Owner user name.
    uname: [u8; 32],
    /// Owner group name.
    gname: [u8; 32],
    /// Device major number (for device files).
    devmajor: [u8; 8],
    /// Device minor number (for device files).
    devminor: [u8; 8],
    /// Filename prefix — prepended to `name` with a '/' separator
    /// for paths longer than 100 characters.
    prefix: [u8; 155],
    /// Padding to fill the 512-byte block.
    _pad: [u8; 12],
}

// Compile-time assertion: header must be exactly 512 bytes.
const _: () = assert!(core::mem::size_of::<TarHeader>() == BLOCK_SIZE);

impl TarHeader {
    /// Returns the filename as a UTF-8 string slice.
    ///
    /// Strips trailing null bytes and slashes. If a prefix is present,
    /// only the base name portion is returned (prefix is ignored for
    /// our simple lookup — we match on the last path component).
    fn name(&self) -> &str {
        let raw = &self.name;
        let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let s = str::from_utf8(&raw[..len]).unwrap_or("");
        s.trim_end_matches('/')
    }

    /// Returns the full path including any USTAR prefix.
    ///
    /// Format: "{prefix}/{name}" if prefix is non-empty, else just "{name}".
    /// Returns the raw bytes for zero-alloc comparison.
    fn prefix(&self) -> &str {
        let raw = &self.prefix;
        let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        str::from_utf8(&raw[..len]).unwrap_or("")
    }

    /// Parses the file size from the octal ASCII `size` field.
    fn file_size(&self) -> usize {
        parse_octal(&self.size)
    }

    /// Returns true if the USTAR magic is present.
    fn is_ustar(&self) -> bool {
        // POSIX: "ustar\0", GNU: "ustar "
        self.magic[0] == b'u'
            && self.magic[1] == b's'
            && self.magic[2] == b't'
            && self.magic[3] == b'a'
            && self.magic[4] == b'r'
    }

    /// Returns true if this is a regular file (type '0' or '\0').
    fn is_regular_file(&self) -> bool {
        self.typeflag == b'0' || self.typeflag == 0
    }

    /// Returns true if the header block is all zeroes (end-of-archive marker).
    fn is_zero(&self) -> bool {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const TarHeader as *const u8,
                BLOCK_SIZE,
            )
        };
        bytes.iter().all(|&b| b == 0)
    }
}

/// Parses an ASCII octal string into a usize.
///
/// TAR encodes numeric fields as null-terminated octal ASCII strings.
/// Example: "0000644\0" → 420 (decimal).
fn parse_octal(field: &[u8]) -> usize {
    let mut result: usize = 0;
    for &byte in field {
        if byte == 0 || byte == b' ' {
            break;
        }
        if byte >= b'0' && byte <= b'7' {
            result = result * 8 + (byte - b'0') as usize;
        }
    }
    result
}

// =============================================================================
// Public API
// =============================================================================

/// A file found in a TAR archive — just a name and a byte slice.
///
/// The `data` slice points directly into the archive memory (zero-copy).
#[derive(Clone, Copy)]
pub struct TarFile<'a> {
    /// Filename (without prefix path).
    pub name: &'a str,
    /// Raw file contents (slice into the original archive).
    pub data: &'a [u8],
}

/// Looks up a file by name in a TAR archive loaded at the given memory region.
///
/// Iterates through the USTAR headers, comparing each filename against
/// `target_name`. Returns the first match as a `TarFile` containing a
/// zero-copy slice into the archive data.
///
/// # Parameters
/// - `archive`: Raw bytes of the entire TAR archive (as loaded by Limine).
/// - `target_name`: Filename to search for (matched against the last path
///   component, e.g., "serial_drv" matches "bin/serial_drv").
///
/// # Returns
/// - `Some(TarFile)` if found.
/// - `None` if the file is not in the archive.
///
/// # Complexity
/// O(n) where n is the number of entries — we scan linearly.
pub fn find_file<'a>(archive: &'a [u8], target_name: &str) -> Option<TarFile<'a>> {
    let mut offset = 0;

    while offset + BLOCK_SIZE <= archive.len() {
        // Overlay the header struct directly onto the archive bytes.
        let header = unsafe {
            &*(archive.as_ptr().add(offset) as *const TarHeader)
        };

        // Two consecutive zero blocks = end of archive.
        if header.is_zero() {
            return None;
        }

        let name = header.name();
        let file_size = header.file_size();

        // Advance past the header to the data blocks.
        let data_offset = offset + BLOCK_SIZE;

        // Check if this is the file we're looking for.
        // Match on the exact name, or on the last path component.
        if header.is_regular_file() {
            let matches = name == target_name
                || name.rsplit('/').next() == Some(target_name);

            if matches && data_offset + file_size <= archive.len() {
                return Some(TarFile {
                    name,
                    data: &archive[data_offset..data_offset + file_size],
                });
            }
        }

        // Advance to the next header: data is padded to 512-byte boundary.
        let data_blocks = (file_size + BLOCK_SIZE - 1) / BLOCK_SIZE;
        offset = data_offset + data_blocks * BLOCK_SIZE;
    }

    None
}

/// Iterates over all regular files in a TAR archive, calling the visitor
/// function for each one.
///
/// Useful for listing the contents of the initrd during boot.
///
/// # Parameters
/// - `archive`: Raw bytes of the TAR archive.
/// - `visitor`: Called for each regular file with `(name, size)`.
pub fn for_each_file<F>(archive: &[u8], mut visitor: F)
where
    F: FnMut(&str, usize),
{
    let mut offset = 0;

    while offset + BLOCK_SIZE <= archive.len() {
        let header = unsafe {
            &*(archive.as_ptr().add(offset) as *const TarHeader)
        };

        if header.is_zero() {
            return;
        }

        let name = header.name();
        let file_size = header.file_size();

        if header.is_regular_file() {
            visitor(name, file_size);
        }

        let data_offset = offset + BLOCK_SIZE;
        let data_blocks = (file_size + BLOCK_SIZE - 1) / BLOCK_SIZE;
        offset = data_offset + data_blocks * BLOCK_SIZE;
    }
}
