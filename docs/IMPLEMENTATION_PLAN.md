# MinimalOS: Multitasking Operating System Implementation Plan

## Overview

This plan outlines the complete implementation of a general-purpose multitasking operating system for x86_64 architecture targeting 64-bit Legacy BIOS systems. The OS will use a microservices architecture with modular, replaceable components following API-driven design principles. The system will boot via GRUB Multiboot2, load binaries from a ramdisk, and provide a Unix-like shell environment.

**Key Design Principles:**
- Microservices architecture with well-defined IPC interfaces
- Modular "lego brick" components with clear input/output contracts
- Type-safe implementations (leveraging C type system and coding standards)
- Clean codebase organization
- Build outputs to `./build`, distribution ISO to `./dist`

---

## Architecture Overview

```mermaid
graph TB
    BIOS[BIOS/Legacy Boot] --> GRUB[GRUB Multiboot2]
    GRUB --> Kernel[Kernel Core]
    
    subgraph "Kernel Space"
        Kernel --> MM[Memory Manager]
        Kernel --> PM[Process Manager]
        Kernel --> Sched[Scheduler]
        Kernel --> Syscall[System Call Interface]
        Kernel --> VFS[Virtual Filesystem]
    end
    
    subgraph "Microservices"
        KBD[Keyboard Driver Service]
        DISK[Disk Driver Service]
        TTY[Terminal Service]
        FS[Filesystem Service]
    end
    
## Phase 7: System Calls & User Mode (COMPLETED)
- [x] **System Call Interface**
  - [x] Syscall handler (using syscall/iretq due to GDT constraints)
  - [x] Syscall table and dispatching
  - [x] Core syscalls (sys_write, sys_exit)

- [x] **User Mode Support**
  - [x] User-space page mapping (fixed permissions)
  - [x] Privilege level transition (ring 0 -> ring 3 via iretq)
  - [x] User stack setup (mapped with user bit)
  - [x] TSS Initialization & Stack Switching (fixed Triple Faults)
subgraph "User Space"
        Shell[Shell Program]
        Apps[User Programs]
    end
    
    MM --> PM
    PM --> Sched
    Sched --> Syscall
    VFS --> FS
    
    KBD -.IPC.-> TTY
    DISK -.IPC.-> FS
    TTY -.IPC.-> Shell
    Syscall --> Shell
    Syscall --> Apps
    
    Kernel --> Ramdisk[Initial Ramdisk]
```

---

## Proposed Changes

### Component 1: Bootloader & Early Boot

#### [NEW] [multiboot2.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/boot/multiboot2.S)
- Multiboot2 header with magic numbers, architecture tag, and end tag
- 16-byte aligned header structure
- Requests for memory map, framebuffer info

#### [NEW] [boot.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/boot/boot.S)
- 32-bit protected mode entry point
- Long mode detection and enablement (check CPUID for x86_64 support)
- Temporary page tables setup (identity mapping first 2MB + higher-half mapping at 0xFFFFFFFF80000000)
- GDT with 64-bit code/data segments
- Jump to 64-bit entry point

#### [NEW] [boot64.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/boot/boot64.S)
- 64-bit long mode entry
- Reload segment selectors
- Setup initial stack (64KB kernel stack)
- Call C kernel entry point with multiboot2 info pointer

#### [NEW] [gdt.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/gdt.c) / [gdt.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/gdt.h)
- GDT structure with null, kernel code/data, user code/data, TSS segments
- GDT loading function
- TSS initialization for syscall/interrupt stack switching

#### [NEW] [idt.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/idt.c) / [idt.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/idt.h)
- IDT structure (256 entries)
- Exception handlers (0-31)
- Interrupt handlers (32-255)
- IDT loading function

#### [NEW] [interrupts.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/interrupts.S)
- ISR stub assembly code (saves all registers, calls C handler, restores registers)
- Exception/Interrupt entry points

---

### Component 2: Kernel Core

#### [NEW] [kernel.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/kernel.c) / [kernel.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/kernel.h)
- `kernel_main()` entry point
- Early initialization sequence:
  1. Parse multiboot2 information
  2. Initialize serial port
  3. Initialize VGA text mode
  4. Initialize GDT/IDT
  5. Initialize memory management
  6. Initialize process management
  7. Load initial ramdisk
  8. Mount root filesystem
  9. Start init process (shell)

#### [NEW] [serial.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/serial.c) / [serial.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/serial.h)
- COM1 serial port driver (port 0x3F8)
- Character output for debugging
- Baud rate configuration (115200)

#### [NEW] [vga.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/vga.c) / [vga.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/vga.h)
- VGA text mode driver (80x25, buffer at 0xB8000)
- Character/string output with colors
- Scrolling support
- Cursor management

#### [NEW] [printk.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/lib/printk.c) / [printk.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/lib/printk.h)
- Kernel printf implementation (`printk()`)
- Format specifiers: %s, %d, %x, %p, %c
- Output to both serial and VGA

#### [NEW] [string.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/lib/string.c) / [string.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/lib/string.h)
- Standard string functions: `memset`, `memcpy`, `memmove`, `strlen`, `strcmp`, `strcpy`, `strncpy`

---

### Component 3: Memory Management

#### [NEW] [pmm.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/pmm.c) / [pmm.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/pmm.h)
**Physical Memory Manager API:**
```c
void pmm_init(multiboot_memory_map_t* mmap, size_t mmap_size);
uintptr_t pmm_alloc_frame(void);        // Allocate single 4KB frame
void pmm_free_frame(uintptr_t frame);   // Free single frame
uintptr_t pmm_alloc_frames(size_t count); // Allocate contiguous frames
void pmm_free_frames(uintptr_t frame, size_t count);
```
- Bitmap allocator for 4KB page frames
- Parse multiboot2 memory map to identify usable RAM
- Reserve kernel and ramdisk regions

#### [NEW] [vmm.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/vmm.c) / [vmm.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/vmm.h)
**Virtual Memory Manager API:**
```c
typedef struct page_directory page_directory_t;

page_directory_t* vmm_create_address_space(void);
void vmm_destroy_address_space(page_directory_t* pd);
void vmm_map_page(page_directory_t* pd, uintptr_t virt, uintptr_t phys, uint32_t flags);
void vmm_unmap_page(page_directory_t* pd, uintptr_t virt);
uintptr_t vmm_get_physical(page_directory_t* pd, uintptr_t virt);
void vmm_switch_directory(page_directory_t* pd);
```
- 4-level paging (PML4 -> PDPT -> PD -> PT)
- Kernel mapped at higher-half (0xFFFFFFFF80000000)
- User space at lower addresses (0x0000000000400000+)
- Page table allocation/deallocation
- Page fault handler

#### [NEW] [heap.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/heap.c) / [heap.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/mm/heap.h)
**Kernel Heap Allocator API:**
```c
void* kmalloc(size_t size);
void* kzalloc(size_t size);           // Zero-initialized allocation
void* krealloc(void* ptr, size_t size);
void kfree(void* ptr);
```
- Slab allocator or simple first-fit allocator
- Metadata for tracking allocations
- Memory leak detection (debug mode)

---

### Component 4: Process Management

#### [NEW] [process.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/process.c) / [process.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/process.h)
**Process Management API:**
```c
typedef enum {
    PROCESS_STATE_READY,
    PROCESS_STATE_RUNNING,
    PROCESS_STATE_BLOCKED,
    PROCESS_STATE_ZOMBIE,
    PROCESS_STATE_TERMINATED
} process_state_t;

typedef struct process {
    uint32_t pid;
    process_state_t state;
    struct cpu_context* context;
    page_directory_t* page_directory;
    struct process* parent;
    list_head_t children;
    int exit_code;
    // File descriptor table
    // IPC endpoints
} process_t;

process_t* process_create(const char* name);
void process_destroy(process_t* proc);
int process_load_elf(process_t* proc, const char* path);
void process_exit(int code);
pid_t process_wait(pid_t pid, int* status);
```

#### [NEW] [scheduler.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/scheduler.c) / [scheduler.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/scheduler.h)
**Scheduler API:**
```c
void scheduler_init(void);
void scheduler_add_process(process_t* proc);
void scheduler_remove_process(process_t* proc);
void schedule(void);  // Called on timer interrupt
void yield(void);     // Voluntary context switch
process_t* get_current_process(void);
```
- Round-robin scheduling with time quantum (10ms)
- Ready queue implementation (circular linked list)
- Process state transitions

#### [NEW] [context.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/context.S) / [context.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/context.h)
**Context Switching:**
```c
typedef struct cpu_context {
    uint64_t r15, r14, r13, r12, rbp, rbx;
    uint64_t rip;  // Return address
    uint64_t rsp;  // Stack pointer
    uint64_t rflags;
    // Segment selectors if needed
} cpu_context_t;

void context_switch(cpu_context_t* old, cpu_context_t* new);
```

#### [NEW] [timer.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/timer.c) / [timer.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/drivers/timer.h)
- PIT (Programmable Interval Timer) driver
- Configure for 100Hz (10ms tick)
- Timer interrupt handler calling scheduler

#### [NEW] [elf.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/loader/elf.c) / [elf.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/loader/elf.h)
**ELF64 Loader API:**
```c
int elf_validate(const void* elf_data);
int elf_load(process_t* proc, const void* elf_data, size_t size);
uintptr_t elf_get_entry(const void* elf_data);
```
- Parse ELF64 header
- Load program segments into process address space
- Setup initial stack and registers (RIP, RSP)

---

### Component 5: Inter-Process Communication (IPC)

#### [NEW] [ipc.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/ipc.c) / [ipc.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/ipc.h)
**Message Passing IPC API:**
```c
typedef struct ipc_message {
    uint32_t sender_pid;
    uint32_t type;        // Message type/opcode
    uint32_t length;      // Data length
    uint8_t data[4096];   // Inline data or shared buffer reference
} ipc_message_t;

typedef struct ipc_endpoint {
    uint32_t endpoint_id;
    process_t* owner;
    queue_t message_queue;
} ipc_endpoint_t;

int ipc_send(uint32_t dest_endpoint, ipc_message_t* msg);
int ipc_receive(uint32_t endpoint, ipc_message_t* msg, bool blocking);
int ipc_create_endpoint(void);  // Returns endpoint ID
void ipc_destroy_endpoint(uint32_t endpoint);
```
- Synchronous message passing (blocking send/receive)
- Message queues per endpoint
- Copy message data between processes

#### [NEW] [shm.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/shm.c) / [shm.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/shm.h)
**Shared Memory API:**
```c
typedef struct shared_region {
    uint32_t id;
    size_t size;
    uintptr_t physical_base;
    // List of processes with access
} shared_region_t;

int shm_create(size_t size);  // Returns region ID
void* shm_attach(int region_id, uintptr_t hint_addr);
int shm_detach(void* addr);
int shm_destroy(int region_id);
```

---

### Component 6: Device Driver Microservices

#### [NEW] [keyboard_driver.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/drivers/keyboard/keyboard_driver.c)
**Keyboard Driver Service (User-space process):**
- PS/2 keyboard driver via port I/O syscalls
- Scancode to ASCII mapping
- IPC interface:
  - Sends keyboard events to terminal service
  - Message format: `{type: KEY_PRESS, keycode: uint8_t, ascii: char}`

#### [NEW] [disk_driver.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/drivers/disk/disk_driver.c)
**Disk Driver Service (User-space process):**
- ATA PIO mode disk I/O
- IPC interface:
  - Request: `{type: DISK_READ/WRITE, lba: uint64_t, count: uint32_t, buffer_shm_id: uint32_t}`
  - Response: `{type: DISK_RESPONSE, status: int, bytes: size_t}`

> [!NOTE]
> For simplicity in initial implementation, keyboard and disk drivers can be kernel-space. Moving to user-space microservices requires syscalls for port I/O and interrupt handling, which adds complexity.

---

### Component 7: Filesystem

#### [NEW] [vfs.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/fs/vfs.c) / [vfs.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/fs/vfs.h)
**Virtual Filesystem API:**
```c
typedef struct vfs_node {
    char name[256];
    uint32_t inode;
    uint32_t type;  // FILE, DIRECTORY, DEVICE
    uint32_t size;
    struct vfs_operations* ops;
    void* fs_private;
} vfs_node_t;

typedef struct vfs_operations {
    int (*read)(vfs_node_t* node, uint64_t offset, size_t size, void* buffer);
    int (*write)(vfs_node_t* node, uint64_t offset, size_t size, const void* buffer);
    vfs_node_t* (*readdir)(vfs_node_t* node, uint32_t index);
    vfs_node_t* (*finddir)(vfs_node_t* node, const char* name);
    int (*open)(vfs_node_t* node);
    void (*close)(vfs_node_t* node);
} vfs_operations_t;

vfs_node_t* vfs_open(const char* path);
int vfs_read(vfs_node_t* node, uint64_t offset, size_t size, void* buffer);
int vfs_write(vfs_node_t* node, uint64_t offset, size_t size, const void* buffer);
void vfs_close(vfs_node_t* node);
```

#### [NEW] [initrd.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/fs/initrd.c) / [initrd.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/fs/initrd.h)
**Initial Ramdisk (TAR format):**
- Parse TAR archive loaded by GRUB as multiboot module
- Extract file metadata (name, size, offset)
- Implement VFS operations for ramdisk files
- Mount ramdisk as root filesystem

---

### Component 8: System Calls

#### [NEW] [syscall.S](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/syscall.S) / [syscall.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/syscall.h)
**Syscall Interface (using `syscall` instruction):**
```c
// Syscall numbers
#define SYS_READ    0
#define SYS_WRITE   1
#define SYS_OPEN    2
#define SYS_CLOSE   3
#define SYS_FORK    4
#define SYS_EXEC    5
#define SYS_EXIT    6
#define SYS_WAIT    7
#define SYS_IPC_SEND   8
#define SYS_IPC_RECV   9
// ... etc

// Syscall handler
void syscall_init(void);  // Setup STAR, LSTAR MSRs
long syscall_handler(long num, long arg1, long arg2, long arg3, long arg4, long arg5);
```

#### [NEW] [syscalls.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/syscalls/syscalls.c)
- Implement syscall handlers:
  - `sys_read(int fd, void* buf, size_t count)`
  - `sys_write(int fd, const void* buf, size_t count)`
  - `sys_open(const char* path, int flags)`
  - `sys_close(int fd)`
  - `sys_fork(void)` - Create child process
  - `sys_exec(const char* path, char* const argv[])` - Load new program
  - `sys_exit(int status)`
  - `sys_wait(pid_t pid, int* status)`

#### [NEW] [fd_table.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/fd_table.c) / [fd_table.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/fd_table.h)
- File descriptor table per process
- Standard streams: stdin (0), stdout (1), stderr (2)
- Map FDs to VFS nodes or IPC endpoints

---

### Component 9: Terminal & Shell

#### [NEW] [terminal.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/services/terminal/terminal.c)
**Terminal Service (User-space process):**
- TTY abstraction with line discipline
- Input buffering and line editing (backspace, enter)
- Echo characters to VGA output
- Connect to keyboard driver via IPC
- Provide character stream to shell via file descriptor

#### [NEW] [shell.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/userspace/shell/shell.c)
**Shell Program:**
- Read command line from stdin
- Parse command and arguments
- Built-in commands:
  - `cd <dir>` - Change directory
  - `pwd` - Print working directory
  - `ls [dir]` - List directory contents
  - `cat <file>` - Display file contents
  - `exit` - Exit shell
- External command execution:
  - `fork()` + `exec()` pattern
  - Wait for child completion
- Simple prompt: `$ `

---

### Component 10: Build System

#### [NEW] [Makefile](file:///media/nadeeshafdo/shared/repos/MinimalOS/Makefile)
**Main Makefile with targets:**
- `all`: Build kernel and ISO
- `kernel`: Compile kernel binary (`build/kernel.elf`)
- `iso`: Create bootable ISO (`dist/minimalos.iso`)
- `ramdisk`: Build initrd.tar with user programs
- `clean`: Remove build artifacts
- `run`: Test in QEMU

**Compiler flags:**
```makefile
CFLAGS = -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
         -mno-mmx -mno-sse -mno-sse2 -mcmodel=kernel \
         -Wall -Wextra -Werror -O2 -g
LDFLAGS = -nostdlib -static -z max-page-size=0x1000 -T linker.ld
```

**Directory structure:**
```
build/
  ├── kernel/         # Compiled kernel object files
  ├── userspace/      # Compiled user programs
  └── ramdisk/        # Ramdisk staging directory
dist/
  └── minimalos.iso   # Final bootable ISO
```

#### [NEW] [linker.ld](file:///media/nadeeshafdo/shared/repos/MinimalOS/linker.ld)
**Linker script:**
- Kernel base at higher-half (0xFFFFFFFF80100000)
- Sections: `.text`, `.rodata`, `.data`, `.bss`
- Align sections to page boundaries
- Export symbols for kernel start/end

#### [NEW] [grub.cfg](file:///media/nadeeshafdo/shared/repos/MinimalOS/iso/boot/grub/grub.cfg)
**GRUB configuration:**
```
set timeout=0
set default=0

menuentry "MinimalOS" {
    multiboot2 /boot/kernel.elf
    module2 /boot/initrd.tar
    boot
}
```

#### Build Process Script
**Ramdisk creation:**
1. Compile user-space programs (shell, drivers)
2. Create directory structure: `/bin`, `/etc`, `/dev`, `/tmp`
3. Copy binaries to staging directory
4. Create TAR archive: `tar -cf build/initrd.tar -C build/ramdisk .`

**ISO creation:**
```bash
mkdir -p iso/boot/grub
cp build/kernel.elf iso/boot/
cp build/initrd.tar iso/boot/
cp grub.cfg iso/boot/grub/
grub-mkrescue -o dist/minimalos.iso iso/
```

---

### Component 11: Type Safety & Code Quality

#### [NEW] [types.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/include/types.h)
**Standard type definitions:**
```c
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

typedef uint64_t uintptr_t;
typedef int64_t intptr_t;
typedef uint64_t size_t;
typedef int64_t ssize_t;
typedef int32_t pid_t;
typedef uint32_t uid_t;
// ... etc
```

#### [NEW] [assert.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/include/assert.h)
**Runtime assertions:**
```c
#define ASSERT(cond) \
    do { if (!(cond)) panic("Assertion failed: " #cond " at " __FILE__ ":" __LINE__); } while(0)

void panic(const char* message);  // Halt system with error
```

#### Code Quality Standards
- Use `const` for read-only parameters
- Use `static` for internal functions
- Avoid global state where possible
- Document all public APIs with comments
- Use meaningful variable/function names
- No magic numbers (use named constants)
- Check all return values
- Validate input parameters

---

## Verification Plan

### Phase 1: Boot Verification
**Test: Boot to kernel entry**
```bash
cd /media/nadeeshafdo/shared/repos/MinimalOS
make clean
make iso
qemu-system-x86_64 -cdrom dist/minimalos.iso -serial stdio -no-reboot -no-shutdown
```
**Expected output:**
- Serial port shows "MinimalOS Booting..."
- VGA text displays kernel initialization messages
- No triple-fault or reboot loop

### Phase 2: Memory Management Verification
**Test: Physical memory allocation**
- Add test code in `kernel_main()`:
  ```c
  uintptr_t frame1 = pmm_alloc_frame();
  uintptr_t frame2 = pmm_alloc_frame();
  printk("Allocated frames: 0x%lx, 0x%lx\n", frame1, frame2);
  pmm_free_frame(frame1);
  uintptr_t frame3 = pmm_alloc_frame();
  printk("Reallocated frame: 0x%lx\n", frame3);
  ```
**Expected:** frame3 should equal frame1 (reused freed frame)

**Test: Virtual memory mapping**
- Map a test page and write/read data
- Verify page fault handler triggers on unmapped access

### Phase 3: Process & Scheduling Verification
**Test: Context switching**
- Create two test processes that print alternating messages
- Verify timer interrupt causes preemption
- Check process states transition correctly

**Test: ELF loading**
- Create simple user program that exits with code 42
- Load and execute from kernel
- Verify process exits with correct code

### Phase 4: Inter-Process Communication (Message Passing)

## Goal Description
Implement a synchronous, copying-based Message Passing interface to allow processes to exchange data. This is the foundation for the microkernel-style driver architecture (Phase 5), where drivers run as user processes and communicate with the OS/clients via IPC.

## User Review Required
> [!NOTE]
> **Blocking Behavior**: `recv` will block the calling process if no messages are available. `send` is currently non-blocking but potentially limited by mailbox size (to be consistent).
> **Data Limit**: Messages are fixed size or capped (e.g., 4KB inline data) to avoid complex memory management initially.

## Proposed Changes

### Kernel Core
#### [MODIFY] [process.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/process.h)
- Add `mailbox` structure to `process_t`.
- `mailbox` will contain a ring buffer or linked list of `ipc_message_t`.

#### [MODIFY] [process.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/process/process.c)
- Initialize mailbox in `process_create`.
- Free mailbox messages in `process_exit`.

### IPC Subsystem
#### [NEW] [ipc.h](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/ipc.h)
- Define `ipc_message_t` struct (sender_pid, type, len, data).
- Define IPC constants (MAX_MSG_SIZE, etc.).

#### [NEW] [ipc.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/ipc/ipc.c)
- `ipc_send(pid_t dest, ipc_message_t* msg)`: Copies message to target mailbox. Wakes up target if blocked.
- `ipc_recv(pid_t from, ipc_message_t* buffer)`: Dequeues message. Blocks if empty.

### System Calls
#### [MODIFY] [syscall.c](file:///media/nadeeshafdo/shared/repos/MinimalOS/src/kernel/arch/x86_64/syscall.c)
- Add handlers for `SYS_IPC_SEND` (8) and `SYS_IPC_RECV` (9).
- Validate user pointers for message buffers.

## Verification Plan
### Automated Tests
- Create a user test program `ipc_test` that forks:
    - **Child**: Sends a "PING" message to Parent.
    - **Parent**: Calls `recv`, blocks, wakes up on "PING", prints it.
### Manual Verification
- Verify logs show specific sequence: `Parent Blocked` -> `Child Sent` -> `Parent Woke` -> `Received PING`.
- Test blocking behavior on empty queue

### Phase 5: Filesystem Verification
**Test: Ramdisk mounting**
- Build ramdisk with test file "hello.txt" containing "Hello, World!"
- Mount ramdisk at boot
- Read file contents via VFS
- Verify output matches original content

**Test: VFS path resolution**
- Test absolute paths: `/bin/shell`
- Test relative paths (if implemented)
- Test error handling for non-existent files

### Phase 6: System Call Verification
**Test: Basic syscalls from user space**
- User program calls `write(1, "test\n", 5)`
- Verify output appears on console
- Test `open()`, `read()`, `close()` on ramdisk file

**Test: Process creation**
- User program calls `fork()`
- Parent waits, child prints message and exits
- Verify parent receives correct exit status

### Phase 7: Shell Integration Test
**Test: Interactive shell**
```bash
make iso
qemu-system-x86_64 -cdrom dist/minimalos.iso -serial stdio
# In shell prompt:
$ pwd
/
$ ls
bin  etc  dev  tmp
$ cat /bin/test.txt
Hello from ramdisk!
$ exit
```
**Expected:** All commands execute successfully

### Phase 8: End-to-End Test
**Test: Execute binary from ramdisk**
- Place compiled "hello" program in ramdisk
- Shell executes: `$ /bin/hello`
- Program prints "Hello, World!" via `write()` syscall
- Program exits cleanly
- Shell returns to prompt

**Success Criteria:**
- System boots without errors
- Shell prompt appears
- User can execute commands
- Binaries from ramdisk execute correctly
- System doesn't crash during normal operation

### Automated Testing (Future Enhancement)
- Unit tests for kernel functions (run on host system)
- Integration test suite with scripted QEMU sessions
- Memory leak detection with custom allocator tracking

---

## Implementation Sequence

1. **Foundation (Weeks 1-2):** Boot process, GDT/IDT, basic drivers (serial, VGA, keyboard)
2. **Memory (Week 3):** Physical allocator, virtual memory, kernel heap
3. **Processes (Weeks 4-5):** Process structures, scheduler, context switching, timer
4. **Loading (Week 6):** ELF loader, initial user process
5. **IPC (Week 7):** Message passing, shared memory
6. **Filesystem (Week 8):** VFS, ramdisk support
7. **Syscalls (Week 9):** Syscall interface, fd table, core syscalls
8. **Shell (Week 10):** Terminal service, shell program
9. **Integration (Week 11):** Build system, ISO creation, end-to-end testing
10. **Refinement (Week 12):** Bug fixes, documentation, optimization

---

## Directory Structure

```
MinimalOS/
├── src/
│   ├── boot/
│   │   ├── multiboot2.S
│   │   ├── boot.S
│   │   └── boot64.S
│   ├── kernel/
│   │   ├── kernel.c
│   │   ├── arch/
│   │   │   └── x86_64/
│   │   │       ├── gdt.c/h
│   │   │       ├── idt.c/h
│   │   │       ├── interrupts.S
│   │   │       ├── context.S/h
│   │   │       └── syscall.S/h
│   │   ├── drivers/
│   │   │   ├── serial.c/h
│   │   │   ├── vga.c/h
│   │   │   ├── keyboard.c/h
│   │   │   ├── timer.c/h
│   │   │   └── disk.c/h
│   │   ├── mm/
│   │   │   ├── pmm.c/h
│   │   │   ├── vmm.c/h
│   │   │   └── heap.c/h
│   │   ├── process/
│   │   │   ├── process.c/h
│   │   │   ├── scheduler.c/h
│   │   │   └── fd_table.c/h
│   │   ├── ipc/
│   │   │   ├── ipc.c/h
│   │   │   └── shm.c/h
│   │   ├── fs/
│   │   │   ├── vfs.c/h
│   │   │   └── initrd.c/h
│   │   ├── loader/
│   │   │   └── elf.c/h
│   │   ├── syscalls/
│   │   │   └── syscalls.c/h
│   │   ├── lib/
│   │   │   ├── printk.c/h
│   │   │   └── string.c/h
│   │   └── include/
│   │       ├── types.h
│   │       └── assert.h
│   ├── userspace/
│   │   ├── shell/
│   │   │   └── shell.c
│   │   ├── lib/
│   │   │   └── libc.c  (minimal C library)
│   │   └── test/
│   │       └── hello.c
│   └── services/
│       └── terminal/
│           └── terminal.c
├── build/          (generated)
├── dist/           (generated)
├── iso/
│   └── boot/
│       └── grub/
│           └── grub.cfg
├── Makefile
└── linker.ld
```

---

## Notes on Microservices Architecture

The microservices architecture is achieved through:

1. **Service Isolation:** Each driver/service runs as separate process with own address space
2. **IPC Contracts:** Well-defined message formats serve as API between services
3. **Service Discovery:** Processes register IPC endpoints with kernel, others lookup by service name
4. **Modularity:** Services can be replaced without kernel changes if they implement same IPC interface
5. **Example:** Replace PS/2 keyboard driver with USB keyboard driver by changing implementation while keeping same message format

**IPC Message Contract Example (Keyboard Service):**
```c
// Input messages TO keyboard service:
// KEYBOARD_GET_EVENT (no data)

// Output messages FROM keyboard service:
typedef struct {
    uint32_t type;      // KEYBOARD_EVENT
    uint8_t scancode;
    uint8_t ascii;
    uint8_t modifiers;  // Shift, Ctrl, Alt flags
} keyboard_event_t;
```

Any service implementing this contract can replace the keyboard driver without system changes.
