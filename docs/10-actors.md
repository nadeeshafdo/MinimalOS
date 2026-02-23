# Wasm Actors

## Actor SDK (`actors/sdk/`)

Shared library linked by all Wasm actors. Provides:

### System Call Wrappers

```rust
extern "C" {
    fn sys_log(ptr: *const u8, len: usize);
    fn sys_exit(code: i32);
    fn sys_cap_send(handle: i64, opcode: i32, ptr: *const u8, len: usize, cap: i64) -> i32;
    fn sys_cap_recv(handle: i64, buf: *mut u8, len: usize) -> i64;
    fn sys_cap_mem_read(handle: i64, offset: i32, buf: *mut u8, len: i32) -> i32;
    fn sys_cap_mem_write(handle: i64, offset: i32, data: *const u8, len: i32) -> i32;
    fn sys_fb_info() -> i64;
}
```

### Helper Functions

- `log(msg: &str)` — wraps `sys_log`
- `cap_send(handle, opcode, payload, cap)` — wraps `sys_cap_send`
- `cap_recv(handle, buf)` — blocking receive, returns sender PID
- `mem_read(handle, offset, buf)` / `mem_write(handle, offset, data)` — shared memory access
- `fb_info() → (width, height)` — unpack framebuffer dimensions

### Memory Allocator

Simple bump allocator for Wasm linear memory:
```rust
static mut HEAP_POS: usize = 0x10000; // Start at 64 KiB offset

pub fn alloc(size: usize) -> *mut u8 {
    // Bump HEAP_POS, align to 8
}
```

### Panic Handler

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log("PANIC in actor");
    sys_exit(1);
    loop {}
}
```

---

## VFS Actor (`actors/vfs/`)

### Purpose
Virtual filesystem service. Indexes a TAR ramdisk and serves file-read requests via IPC.

### Capability Slots (post-spawn)

| Slot | Kind | Target | Permissions | Description |
|---|---|---|---|---|
| 0 | IpcEndpoint | self | READ | Receive requests |
| 1 | Memory | ramdisk buffer | READ | Read ramdisk contents |

### TAR Indexing

On startup, VFS reads the entire ramdisk via `sys_cap_mem_read` and builds a file index:
```rust
struct FileEntry {
    name: [u8; 100],
    offset: usize,     // Byte offset in ramdisk
    size: usize,       // File size from TAR header
}

static mut FILES: [FileEntry; 32] = ...; // Max 32 files
```

TAR header parsing:
- 512-byte headers, octal size field at offset 124
- Skip special entries (directories, `./`, `pax_global_header`)
- Data follows header, padded to 512-byte boundary

### Service Loop

```
loop:
  1. cap_recv(self_endpoint) → Message
  2. opcode 1 = "list files" → reply with file count + names
  3. opcode 2 = "read file" → extract filename from payload
     a. Find in FILES index
     b. Read file data from ramdisk memory cap
     c. Reply with file contents (chunked if > 16 bytes)
```

### Reply Routing

VFS uses the capability transfer convention:
- Client sends IPC with `cap_slot` = client's own endpoint handle (GRANT permission)
- VFS receives → `cap_slot` is now a handle in VFS's CapTable pointing back to client
- VFS sends reply via that handle

---

## UI Server Actor (`actors/ui_server/`)

### Purpose
Composites a graphical display: renders text, draws cursor, handles input events via IPC.

### Capability Slots (post-spawn)

| Slot | Kind | Target | Permissions | Description |
|---|---|---|---|---|
| 0 | IpcEndpoint | self | READ | Receive draw commands |
| 1 | Framebuffer | fb addr | READ+WRITE | Direct pixel access |
| 2 | Memory | font buffer | READ | PSF font glyph data |

### PSF Font Loading

On startup:
1. Read PSF v2 header (32 bytes) from font memory capability
2. Extract: `glyph_count`, `glyph_size`, `height`, `width`, `header_size`
3. Read all glyph bitmaps into local buffer

### Rendering

- **`draw_char(x, y, ch, color)`**: Blit 1-bit glyph to framebuffer via `sys_cap_mem_write`
- **`draw_string(x, y, text, color)`**: Iterate chars, advance x by glyph width
- **`fill_rect(x, y, w, h, color)`**: Solid color rectangle
- **Resolution**: Hardcoded 1280×800, 32bpp BGRA

### IPC Protocol

| Opcode | Payload | Action |
|---|---|---|
| 1 | `[x: u16, y: u16, color: u32, ...chars]` | Draw string at position |
| 2 | `[x: u16, y: u16, w: u16, h: u16, color: u32]` | Fill rectangle |
| 3 | (none) | Redraw/flush (no-op currently) |

### Service Loop

```
loop:
  1. cap_recv(self_endpoint) → Message
  2. Dispatch on opcode → draw_char / fill_rect / etc.
  3. No reply needed (fire-and-forget rendering)
```

---

## Shell Actor (`actors/shell/`)

### Purpose
Demo application — reads a file from VFS and displays it via UI Server.

### Capability Slots (post-spawn)

| Slot | Kind | Target | Permissions | Description |
|---|---|---|---|---|
| 0 | IpcEndpoint | self | READ+WRITE+GRANT | Own endpoint (for receiving replies) |
| 1 | IpcEndpoint | VFS | WRITE | Send requests to VFS |
| 2 | IpcEndpoint | UI Server | WRITE | Send draw commands to UI |

### Startup Sequence

```
_start():
  1. log("Shell actor started")
  2. Send IPC to VFS (opcode 2, filename "hello.txt") with cap_slot = own endpoint
  3. Recv reply from VFS → file contents
  4. Send IPC to UI Server (opcode 1) → draw file contents at (10, 10)
  5. log("Shell: done")
  6. sys_exit(0)
```

### Interaction Flow

```
Shell                    VFS                     UI Server
  |                       |                         |
  |--[read hello.txt]---->|                         |
  |  (cap_slot=own_ep)    |                         |
  |                       |--[mem_read ramdisk]-->  |
  |<--[file contents]-----|                         |
  |                                                 |
  |--[draw text opcode 1]------------------------->|
  |                                                 |--[write to FB]
  |--[sys_exit(0)]                                  |
```

---

## Actor Lifecycle Summary

```
Boot:
  1. Kernel extracts .wasm files from ramdisk TAR
  2. spawn_wasm() for each actor (VFS, UI Server, Shell)
  3. Kernel grants initial capabilities
  4. Kernel cross-grants IPC endpoints between actors
  5. Scheduler starts running actors

Runtime:
  - Actors communicate exclusively via IPC messages
  - Actors access hardware only through capabilities (framebuffer, memory)
  - Kernel mediates all cross-actor interactions

Shutdown:
  - Actor calls sys_exit() → Process marked Dead
  - Next do_schedule() drops the dead process
  - Capabilities are not currently revoked on death (known limitation)
```
