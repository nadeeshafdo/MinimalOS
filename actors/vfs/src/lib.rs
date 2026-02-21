#![no_std]
#![no_main]

use actor_sdk::{log, mem_read};

const RAMDISK_CAP: u64 = 1;

#[derive(Copy, Clone)]
struct TarFile {
    name: [u8; 64],
    offset: u64,
    size: usize,
}

// Track up to 32 files in our zero-allocation static array.
static mut FILES: [TarFile; 32] = [TarFile { name: [0; 64], offset: 0, size: 0 }; 32];

fn parse_octal(bytes: &[u8]) -> usize {
    let mut sum = 0;
    for &b in bytes.iter() {
        if b >= b'0' && b <= b'7' {
            sum = sum * 8 + (b - b'0') as usize;
        }
    }
    sum
}

#[no_mangle]
pub extern "C" fn _start() {
    log!("VFS Actor Started — initializing ZAFS (Zero-Allocation FS)...");

    let mut block = [0u8; 512];
    let mut offset = 0;
    let mut file_count = 0;

    loop {
        if mem_read(RAMDISK_CAP, offset, &mut block).is_err() {
            log!("VFS: Read error at offset {}, stopping.", offset);
            break;
        }

        // EOF is marked by two empty 512-byte blocks. Just check first byte.
        if block[0] == 0 {
            break;
        }

        // USTAR header structure:
        // offset 0..100: name
        // offset 124..136: size in octal ascii
        // offset 156: typeflag (0 or '0' = file, 5 = dir)
        // offset 257..263: "ustar\0" magic marker

        let size_str = &block[124..135]; // omit null terminator
        let file_size = parse_octal(size_str);
        let typeflag = block[156];

        // Ensure to advance offset for the header itself
        let data_offset = offset + 512;

        if typeflag == 0 || typeflag == b'0' { // Regular File
            if file_count < 32 {
                let mut name = [0u8; 64];
                for i in 0..64 {
                    if block[i] == 0 { break; }
                    name[i] = block[i];
                }

                unsafe {
                    FILES[file_count] = TarFile {
                        name,
                        offset: data_offset,
                        size: file_size,
                    };
                }
                
                if let Ok(name_str) = core::str::from_utf8(&unsafe { FILES[file_count].name }) {
                    log!("VFS Index: [{}] '{}' ({} bytes, @{:#x})", file_count, name_str.trim_matches('\0'), file_size, data_offset);
                }
                
                file_count += 1;
            }
        }

        // Advance to next header. Headers are padded to 512 bytes.
        let bytes_to_skip = 512 + ((file_size + 511) / 512) * 512;
        offset += bytes_to_skip as u64;
    }

    log!("VFS: ZAFS initialization complete. Tracking {} files.", file_count);
    
    // Future phase: Enter sys_cap_recv loop here to serve read requests.
    loop {
        // Wait forever in Phase 7
        actor_sdk::exit(0);
    }
}
