#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::log;

const RAMDISK_CAP: u64 = 1;

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

fn parse_string(bytes: &[u8]) -> &str {
    let mut len = 0;
    while len < bytes.len() && bytes[len] != 0 {
        len += 1;
    }
    core::str::from_utf8(&bytes[..len]).unwrap_or("<invalid utf8>")
}

#[no_mangle]
pub extern "C" fn _start() {
    log!("VFS Actor started. Initializing Zero-Allocation File System...");

    let mut block = [0u8; 512];
    let mut offset = 0;
    let mut file_count = 0;

    loop {
        // Blit 512 bytes (one block) from the RAMDisk capability into our stack
        let res = unsafe { sdk::sys_cap_mem_read(RAMDISK_CAP as i64, offset, block.as_mut_ptr() as i32, 512) };
        if res != 0 {
            log!("VFS: Read error at offset {}", offset);
            break;
        }

        // A block starting with a null byte indicates the end of the archive
        if block[0] == 0 {
            break;
        }

        // Validate USTAR magic ("ustar" or "ustar\0")
        if &block[257..262] != b"ustar" {
            log!("VFS: Invalid TAR magic at offset {}", offset);
            break;
        }

        let name = parse_string(&block[0..100]);
        let size = parse_octal(&block[124..136]);
        let type_flag = block[156];

        // Type '0' is a regular file, '5' is a directory
        if type_flag == b'0' || type_flag == 0 {
            log!("VFS File {}: '{}' ({} bytes) at offset {}", file_count, name, size, offset + 512);
            file_count += 1;
        }

        // Advance offset: 1 header block + N data blocks (padded to 512)
        let data_blocks = (size + 511) / 512;
        offset += 512 + (data_blocks * 512) as i32;
    }

    log!("VFS: Initialization complete. {} files indexed. Yielding...", file_count);

    // Tell the kernel this actor is done for now
    unsafe { sdk::sys_exit(0); }
}
