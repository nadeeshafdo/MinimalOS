#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::{log, Message};

/// RAMDisk Memory capability (slot 1, seeded by kernel with READ|GRANT).
const RAMDISK_CAP: i64 = 1;
/// Endpoint to Shell actor (slot 2, seeded by kernel post-spawn).
const EP_SHELL: i64 = 2;
/// Endpoint to UI Server actor (slot 3, seeded by kernel post-spawn).
const EP_UI: i64 = 3;
/// Permission bit: READ.
const PERM_READ: u32 = 1 << 0;

/// Maximum files we can index from the TAR archive.
const MAX_FILES: usize = 32;

/// A file index entry — name, offset in ramdisk, and size.
struct FileEntry {
    name: [u8; 100],
    name_len: usize,
    /// Byte offset of file data within the ramdisk (header + 512).
    offset: u32,
    size: u32,
}

fn parse_octal(bytes: &[u8]) -> usize {
    let mut val = 0;
    for &b in bytes {
        if b >= b'0' && b <= b'7' {
            val = (val << 3) | ((b - b'0') as usize);
        } else if b == 0 || b == b' ' {
            break;
        }
    }
    val
}

fn parse_string_len(bytes: &[u8]) -> usize {
    let mut len = 0;
    while len < bytes.len() && bytes[len] != 0 {
        len += 1;
    }
    len
}

/// Unpack a null-terminated filename from Message.data (24 bytes as [u64; 3]).
fn unpack_filename(data: &[u64; 3]) -> ([u8; 24], usize) {
    let mut buf = [0u8; 24];
    let b0 = data[0].to_le_bytes();
    let b1 = data[1].to_le_bytes();
    let b2 = data[2].to_le_bytes();
    buf[0..8].copy_from_slice(&b0);
    buf[8..16].copy_from_slice(&b1);
    buf[16..24].copy_from_slice(&b2);
    let len = parse_string_len(&buf);
    (buf, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("VFS Actor started. Initializing Zero-Allocation File System...");

    // ── Phase 1: Index all files from the TAR archive ──────────
    let mut files: [FileEntry; MAX_FILES] = unsafe { core::mem::zeroed() };
    let mut file_count = 0usize;
    let mut block = [0u8; 512];
    let mut offset: i32 = 0;

    loop {
        let res = unsafe { sdk::sys_cap_mem_read(RAMDISK_CAP, offset, block.as_mut_ptr() as i32, 512) };
        if res != 0 {
            log!("VFS: Read error at offset {}", offset);
            break;
        }

        if block[0] == 0 {
            break;
        }

        if &block[257..262] != b"ustar" {
            log!("VFS: Invalid TAR magic at offset {}", offset);
            break;
        }

        let name_len = parse_string_len(&block[0..100]);
        let size = parse_octal(&block[124..136]);
        let type_flag = block[156];

        if (type_flag == b'0' || type_flag == 0) && file_count < MAX_FILES {
            let entry = &mut files[file_count];
            // Strip leading "./" prefix from TAR filenames.
            let (name_start, effective_len) = if name_len >= 2 && block[0] == b'.' && block[1] == b'/' {
                (2usize, name_len - 2)
            } else {
                (0usize, name_len)
            };
            let copy_len = effective_len.min(100);
            entry.name[..copy_len].copy_from_slice(&block[name_start..name_start + copy_len]);
            entry.name_len = copy_len;
            entry.offset = (offset + 512) as u32;
            entry.size = size as u32;

            if let Ok(name) = core::str::from_utf8(&entry.name[..copy_len]) {
                log!("VFS: indexed [{}] '{}' ({} bytes @ {})", file_count, name, size, offset + 512);
            }
            file_count += 1;
        }

        let data_blocks = (size + 511) / 512;
        offset += 512 + (data_blocks * 512) as i32;
    }

    log!("VFS: {} files indexed. Entering service loop...", file_count);

    // ── Phase 2: Service loop — wait for IPC requests ──────────
    loop {
        let mut msg = Message::empty();
        let recv_result = unsafe { sdk::sys_cap_recv(&mut msg as *mut Message as i32) };
        if recv_result != 0 {
            log!("VFS: recv error ({})", recv_result);
            continue;
        }

        match msg.label {
            sdk::VFS_READ_REQ => {
                let (name_buf, name_len) = unpack_filename(&msg.data);

                if name_len == 0 {
                    log!("VFS: READ_REQ with empty filename");
                    continue;
                }

                let requested = &name_buf[..name_len];

                // Search the index.
                let mut found = false;
                for i in 0..file_count {
                    let entry = &files[i];
                    if entry.name_len == name_len && &entry.name[..name_len] == requested {
                        // Determine reply endpoint from data[2] hint.
                        // Requestors encode which VFS EP slot points back
                        // to them in data[2] (safe because filenames are
                        // < 16 chars, so data[2] is unused by filename).
                        let reply_ep = match msg.data[2] {
                            3 => EP_UI,
                            _ => EP_SHELL, // default: Shell (slot 2)
                        };

                        if let Ok(name) = core::str::from_utf8(requested) {
                            log!("VFS: READ_REQ '{}' -> offset={}, size={}, reply_ep={}",
                                 name, entry.offset, entry.size, reply_ep);
                        }

                        // Build reply: offset + size in data, grant READ-only ramdisk cap.
                        let reply = Message {
                            label: sdk::VFS_READ_REPLY,
                            data: [entry.offset as u64, entry.size as u64, 0],
                            cap_grant: RAMDISK_CAP as u64,   // grant our ramdisk cap
                            cap_perms: PERM_READ,             // narrow to READ only
                            _pad: 0,
                        };
                        let send_result = unsafe {
                            sdk::sys_cap_send(reply_ep, &reply as *const Message as i32)
                        };
                        if send_result != 0 {
                            log!("VFS: ERROR — failed to send reply ({})", send_result);
                        }
                        found = true;
                        break;
                    }
                }

                if !found {
                    if let Ok(name) = core::str::from_utf8(requested) {
                        log!("VFS: file not found: '{}'", name);
                    }
                    let reply_ep = match msg.data[2] {
                        3 => EP_UI,
                        _ => EP_SHELL,
                    };
                    // Send error reply (size=0, no cap).
                    let reply = Message {
                        label: sdk::VFS_READ_REPLY,
                        data: [0, 0, 0],
                        cap_grant: 0,
                        cap_perms: 0,
                        _pad: 0,
                    };
                    let _ = unsafe { sdk::sys_cap_send(reply_ep, &reply as *const Message as i32) };
                }
            }
            other => {
                log!("VFS: unknown message label {}", other);
            }
        }
    }
}
