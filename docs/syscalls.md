---
layout: default
title: Syscall Reference
---

# Syscall Reference

## Overview

MinimalOS uses the x86_64 `syscall` / `sysret` instruction pair for user-to-kernel
transitions. The syscall interface is enabled via the EFER MSR and configured
during boot.

### Convention

| Register | Purpose |
|----------|---------|
| `rax` | Syscall number |
| `rdi` | Argument 1 |
| `rsi` | Argument 2 |
| `rdx` | Argument 3 |
| `rax` | Return value |
| `rcx` | Clobbered (holds return RIP) |
| `r11` | Clobbered (holds return RFLAGS) |

### MSR Configuration

| MSR | Value | Purpose |
|-----|-------|---------|
| `EFER` (0xC0000080) | bit 0 set | Enable `syscall`/`sysret` (SCE) |
| `LSTAR` (0xC0000082) | `syscall_entry` | Kernel entry point address |
| `STAR` (0xC0000081) | `0x0010_0008_0000_0000` | Segment selectors for CS/SS |
| `SFMASK` (0xC0000084) | `0x200` | Mask IF on syscall entry |

### Entry Stub

The `syscall_entry` assembly stub:

1. Swaps to the kernel stack (stored in a per-CPU variable `SYSCALL_KERNEL_RSP`).
2. Saves all general-purpose registers to the kernel stack.
3. Calls the Rust `handle_syscall(rax, rdi, rsi, rdx)` dispatcher.
4. Restores registers.
5. Executes `sysretq` to return to Ring 3.

---

## Syscall Table

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| 0 | `SYS_LOG` | `log(ptr, len)` | Write a string to serial output |
| 1 | `SYS_EXIT` | `exit(code)` | Terminate the current process |
| 2 | `SYS_YIELD` | `yield()` | Voluntarily give up the CPU |
| 3 | `SYS_SPAWN` | `spawn(path_ptr, path_len)` | Launch a new process from the ramdisk |
| 4 | `SYS_READ` | `read(fd, buf, len) → count` | Read bytes from a file descriptor |

---

## SYS_LOG (0)

**Write a message to the kernel serial log.**

### Parameters

| Register | Type | Description |
|----------|------|-------------|
| `rdi` | `*const u8` | Pointer to UTF-8 string in user memory |
| `rsi` | `usize` | Length in bytes |

### Return Value

`rax` = 0 (always succeeds)

### Behaviour

The kernel reads `len` bytes from the user-space pointer and writes them to
the COM1 serial port via `klog`. No newline is appended — the user program
should include one if desired.

### Example (User-Space)

```rust
fn sys_log(msg: &str) {
	unsafe {
		core::arch::asm!(
			"syscall",
			in("rax") 0u64,
			in("rdi") msg.as_ptr() as u64,
			in("rsi") msg.len() as u64,
			out("rcx") _,
			out("r11") _,
		);
	}
}

sys_log("Hello from userspace!\n");
```

---

## SYS_EXIT (1)

**Terminate the calling process.**

### Parameters

| Register | Type | Description |
|----------|------|-------------|
| `rdi` | `u64` | Exit code (logged but not otherwise used) |

### Return Value

Does not return.

### Behaviour

1. Logs the exit code and process name to serial.
2. Sets the process state to `Terminated`.
3. Calls `do_schedule()` to switch to the next ready process.
4. The terminated process's resources will be reclaimed when the scheduler
   encounters it.

### Example

```rust
fn sys_exit(code: u64) -> ! {
	unsafe {
		core::arch::asm!(
			"syscall",
			in("rax") 1u64,
			in("rdi") code,
			out("rcx") _,
			out("r11") _,
			options(noreturn),
		);
	}
}
```

---

## SYS_YIELD (2)

**Voluntarily yield the CPU to the next ready process.**

### Parameters

None.

### Return Value

`rax` = 0 (after the process is re-scheduled)

### Behaviour

Calls `do_schedule()`, which performs a round-robin context switch. The calling
process is moved to the back of the ready queue and will eventually be
re-scheduled.

### Example

```rust
fn sys_yield() {
	unsafe {
		core::arch::asm!(
			"syscall",
			in("rax") 2u64,
			out("rcx") _,
			out("r11") _,
		);
	}
}
```

---

## SYS_SPAWN (3)

**Launch a new process from an ELF binary on the ramdisk.**

### Parameters

| Register | Type | Description |
|----------|------|-------------|
| `rdi` | `*const u8` | Pointer to the filename string (e.g., `"shell.elf"`) |
| `rsi` | `usize` | Length of the filename |

### Return Value

| Value | Meaning |
|-------|---------|
| PID (> 0) | Process created successfully |
| 0 | Failure (file not found, ELF parse error, etc.) |

### Behaviour

1. Reads the filename from user memory.
2. Searches the ramdisk TAR archive for a matching file.
3. Parses the ELF header and program headers.
4. Maps `PT_LOAD` segments into user-space pages.
5. Allocates a user stack at `0x800000 + pid * 0x10000`.
6. Creates a `Process` with a 32 KiB kernel stack.
7. Pushes the process to the scheduler's ready queue.
8. Returns the new process's PID.

### Example

```rust
fn sys_spawn(name: &str) -> u64 {
	let pid: u64;
	unsafe {
		core::arch::asm!(
			"syscall",
			in("rax") 3u64,
			in("rdi") name.as_ptr() as u64,
			in("rsi") name.len() as u64,
			lateout("rax") pid,
			out("rcx") _,
			out("r11") _,
		);
	}
	pid
}
```

---

## SYS_READ (4)

**Read bytes from a file descriptor.**

### Parameters

| Register | Type | Description |
|----------|------|-------------|
| `rdi` | `u64` | File descriptor (currently only `0` = stdin) |
| `rsi` | `*mut u8` | Buffer to read into |
| `rdx` | `usize` | Maximum number of bytes to read |

### Return Value

| Value | Meaning |
|-------|---------|
| > 0 | Number of bytes read |
| 0 | No data available (non-blocking) |
| -1 (`u64::MAX`) | Invalid file descriptor |

### Behaviour

When `fd = 0` (stdin):

1. The kernel checks the keyboard input ring buffer.
2. If data is available, copies up to `len` bytes into the user buffer.
3. Returns the number of bytes copied.
4. If no data is available, returns 0 (non-blocking).

The user-mode shell uses a polling loop with `sys_yield()` between reads to
wait for keyboard input without busy-waiting.

### Example

```rust
fn sys_read(fd: u64, buf: &mut [u8]) -> u64 {
	let count: u64;
	unsafe {
		core::arch::asm!(
			"syscall",
			in("rax") 4u64,
			in("rdi") fd,
			in("rsi") buf.as_mut_ptr() as u64,
			in("rdx") buf.len() as u64,
			lateout("rax") count,
			out("rcx") _,
			out("r11") _,
		);
	}
	count
}
```

---

## Keyboard Input Buffer

**File:** `kernel/src/task/input.rs`

The kernel maintains a 256-byte ring buffer for keyboard input:

```rust
pub fn push(byte: u8)		   // Called from keyboard IRQ handler
pub fn pop() -> Option<u8>	  // Called from SYS_READ handler
```

The buffer uses `head` and `tail` indices protected by a spinlock.
When the buffer is full, new keystrokes are silently dropped.
