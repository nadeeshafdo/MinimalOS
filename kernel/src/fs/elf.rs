//! Minimal ELF64 parser for loading user-mode executables.
//!
//! Supports loading statically-linked ELF64 executables with PT_LOAD segments.
//! Only the subset needed to load flat user binaries is implemented.

/// ELF magic number: 0x7f 'E' 'L' 'F'.
const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// ELF class: 64-bit.
const ELFCLASS64: u8 = 2;

/// ELF data encoding: little-endian.
const ELFDATA2LSB: u8 = 1;

/// ELF type: executable.
const ET_EXEC: u16 = 2;

/// ELF machine: x86-64.
const EM_X86_64: u16 = 62;

/// Program header type: loadable segment.
const PT_LOAD: u32 = 1;

/// Program header flags.
#[allow(dead_code)]
const PF_X: u32 = 1;      // Execute
#[allow(dead_code)]
const PF_W: u32 = 2;      // Write
const _PF_R: u32 = 4;     // Read

/// ELF64 file header (first 64 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

/// ELF64 program header (56 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

/// Information extracted from a validated ELF64 executable.
pub struct ElfInfo<'a> {
    /// Raw ELF data.
    pub data: &'a [u8],
    /// Entry point virtual address.
    pub entry: u64,
    /// Program headers.
    pub phdrs: &'a [Elf64Phdr],
}

/// Errors that can occur during ELF parsing.
#[derive(Debug)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    Not64Bit,
    NotLittleEndian,
    NotExecutable,
    NotX86_64,
    BadPhdr,
}

/// Parse and validate an ELF64 executable from a byte slice.
pub fn parse(data: &[u8]) -> Result<ElfInfo<'_>, ElfError> {
    if data.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::TooSmall);
    }

    // SAFETY: we checked size; the struct is packed so alignment is 1.
    let hdr = unsafe { &*(data.as_ptr() as *const Elf64Header) };

    if hdr.e_ident[0..4] != ELF_MAGIC {
        return Err(ElfError::BadMagic);
    }
    if hdr.e_ident[4] != ELFCLASS64 {
        return Err(ElfError::Not64Bit);
    }
    if hdr.e_ident[5] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }
    if hdr.e_type != ET_EXEC {
        return Err(ElfError::NotExecutable);
    }
    if hdr.e_machine != EM_X86_64 {
        return Err(ElfError::NotX86_64);
    }

    let phoff = hdr.e_phoff as usize;
    let phnum = hdr.e_phnum as usize;
    let phentsize = hdr.e_phentsize as usize;

    if phentsize != core::mem::size_of::<Elf64Phdr>() {
        return Err(ElfError::BadPhdr);
    }

    let phdrs_end = phoff + phnum * phentsize;
    if phdrs_end > data.len() {
        return Err(ElfError::BadPhdr);
    }

    // SAFETY: bounds checked, packed struct, alignment 1.
    let phdrs = unsafe {
        core::slice::from_raw_parts(
            data.as_ptr().add(phoff) as *const Elf64Phdr,
            phnum,
        )
    };

    Ok(ElfInfo {
        data,
        entry: hdr.e_entry,
        phdrs,
    })
}

/// Convert ELF segment flags to page flags.
///
/// Returns `(user_rw, executable)`.
#[allow(dead_code)]
pub fn segment_flags(p_flags: u32) -> (bool, bool) {
    let writable = (p_flags & PF_W) != 0;
    let executable = (p_flags & PF_X) != 0;
    let _ = writable; // currently we map all user pages as USER_RW
    (true, executable)
}

impl Elf64Phdr {
    /// Returns true if this is a PT_LOAD segment.
    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }
}
