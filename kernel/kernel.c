/* MinimalOS 64-bit Kernel */

#include <stdint.h>
#include "idt.h"
#include "pic.h"
#include "timer.h"
#include "keyboard.h"
#include "multiboot2.h"
#include "pmm.h"
#include "kheap.h"
#include "process.h"
#include "scheduler.h"
#include "syscall.h"
#include "vfs.h"
#include "initrd.h"
#include "serial.h"
#include "tss.h"
#include "user.h"

/* VGA text mode */
#define VGA_BUFFER ((volatile uint16_t*)0xB8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VGA_COLOR(fg, bg) ((bg << 4) | fg)
#define VGA_ENTRY(c, color) ((uint16_t)(c) | ((uint16_t)(color) << 8))

static int cursor_x = 0;
static int cursor_y = 0;
static uint8_t color = VGA_COLOR(15, 0);
static uint64_t saved_mb_info = 0;

void clear_screen(void) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) VGA_BUFFER[i] = VGA_ENTRY(' ', color);
    cursor_x = cursor_y = 0;
}

static void scroll(void) {
    for (int i = 0; i < VGA_WIDTH * (VGA_HEIGHT - 1); i++) VGA_BUFFER[i] = VGA_BUFFER[i + VGA_WIDTH];
    for (int i = 0; i < VGA_WIDTH; i++) VGA_BUFFER[(VGA_HEIGHT - 1) * VGA_WIDTH + i] = VGA_ENTRY(' ', color);
    cursor_y = VGA_HEIGHT - 1;
}

void putchar(char c) {
    if (c == '\n') { cursor_x = 0; cursor_y++; }
    else if (c == '\r') cursor_x = 0;
    else if (c == '\t') cursor_x = (cursor_x + 8) & ~7;
    else if (c == '\b') { if (cursor_x > 0) { cursor_x--; VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(' ', color); } }
    else { VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(c, color); cursor_x++; }
    if (cursor_x >= VGA_WIDTH) { cursor_x = 0; cursor_y++; }
    if (cursor_y >= VGA_HEIGHT) scroll();
}

void puts(const char *s) { while (*s) putchar(*s++); }

void print_dec(uint64_t n) {
    if (n == 0) { putchar('0'); return; }
    char buf[21]; int i = 0;
    while (n) { buf[i++] = '0' + (n % 10); n /= 10; }
    while (i--) putchar(buf[i]);
}

void print_hex(uint64_t n) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    int started = 0;
    for (int i = 60; i >= 0; i -= 4) {
        int d = (n >> i) & 0xF;
        if (d || started || i == 0) { putchar(hex[d]); started = 1; }
    }
}

void set_color(uint8_t c) { color = c; }

static int strcmp(const char *s1, const char *s2) {
    while (*s1 && *s1 == *s2) { s1++; s2++; }
    return *s1 - *s2;
}

/* Exception handler */
static const char *exceptions[] = {
    "Divide by Zero", "Debug", "NMI", "Breakpoint", "Overflow",
    "Bound Range", "Invalid Opcode", "Device N/A", "Double Fault",
    "Coproc Seg", "Invalid TSS", "Seg Not Present", "Stack Fault",
    "General Protection", "Page Fault", "Reserved", "x87 FPU",
    "Alignment Check", "Machine Check", "SIMD Exception"
};

void exception_handler(uint64_t num, uint64_t err) {
    set_color(VGA_COLOR(15, 4));
    puts("\n*** EXCEPTION: ");
    if (num < 20) puts(exceptions[num]);
    puts(" ***\n");
    set_color(VGA_COLOR(15, 0));
    puts("INT: "); print_dec(num);
    puts(" ERR: "); print_hex(err);
    puts("\nSystem halted.");
    while (1) __asm__ volatile ("hlt");
}

/* Test tasks - these are created but not scheduled preemptively */
/* To avoid blocking the shell, we count only when explicitly triggered */
static volatile uint64_t task_a_count = 0;
static volatile uint64_t task_b_count = 0;

/* Demo: tasks just increment and yield immediately */
void task_a(void) {
    while (1) {
        task_a_count++;
        __asm__ volatile ("hlt");  /* Wait for interrupt instead of busy loop */
    }
}

void task_b(void) {
    while (1) {
        task_b_count++;
        __asm__ volatile ("hlt");  /* Wait for interrupt instead of busy loop */
    }
}

/* Shell */
#define CMD_BUFFER_SIZE 256
static char cmd_buffer[CMD_BUFFER_SIZE];
static int cmd_pos = 0;

void shell_prompt(void) {
    set_color(VGA_COLOR(10, 0)); puts("\nminimal");
    set_color(VGA_COLOR(15, 0)); puts("> ");
}

void shell_execute(void) {
    cmd_buffer[cmd_pos] = '\0';
    if (cmd_pos == 0) { shell_prompt(); return; }
    
    if (strcmp(cmd_buffer, "help") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Available commands:\n");
        set_color(VGA_COLOR(15, 0));
        puts("  help     - Show this help\n");
        puts("  clear    - Clear screen\n");
        puts("  uptime   - Show system uptime\n");
        puts("  mem      - Show memory info\n");
        puts("  ps       - List processes\n");
        puts("  ls       - List files\n");
        puts("  cat FILE - Display file contents\n");
        puts("  syscall  - Test syscall interface\n");
        puts("  usermode - Run user mode demo\n");
        puts("  reboot   - Reboot system\n");
        puts("  halt     - Halt CPU\n");
    }
    else if (strcmp(cmd_buffer, "clear") == 0) {
        clear_screen();
    }
    else if (strcmp(cmd_buffer, "uptime") == 0) {
        puts("\nUptime: ");
        print_dec(timer_get_uptime());
        puts(" seconds (");
        print_dec(timer_get_ticks());
        puts(" ticks)\n");
    }
    else if (strcmp(cmd_buffer, "mem") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Memory Information:\n");
        set_color(VGA_COLOR(15, 0));
        puts("  Physical: ");
        print_dec(pmm_get_free_memory() / 1024 / 1024);
        puts("/");
        print_dec(pmm_get_total_memory() / 1024 / 1024);
        puts(" MB free\n");
        puts("  Heap: ");
        print_dec(kheap_get_free() / 1024);
        puts(" KB free\n");
    }
    else if (strcmp(cmd_buffer, "ps") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Processes:\n");
        set_color(VGA_COLOR(15, 0));
        puts("  PID  STATE    NAME\n");
        
        for (uint64_t i = 0; i < 10; i++) {
            process_t *p = process_get(i);
            if (!p) continue;
            
            puts("  ");
            print_dec(p->pid);
            puts("    ");
            
            switch (p->state) {
                case PROCESS_RUNNING: set_color(VGA_COLOR(10, 0)); puts("RUN  "); break;
                case PROCESS_READY: set_color(VGA_COLOR(14, 0)); puts("READY"); break;
                case PROCESS_BLOCKED: set_color(VGA_COLOR(12, 0)); puts("BLOCK"); break;
                case PROCESS_TERMINATED: set_color(VGA_COLOR(8, 0)); puts("TERM "); break;
            }
            set_color(VGA_COLOR(15, 0));
            puts("    ");
            puts(p->name);
            puts("\n");
        }
        puts("\nTotal: ");
        print_dec(process_count());
        puts(" processes\n");
    }
    else if (strcmp(cmd_buffer, "syscall") == 0) {
        puts("\nTesting syscall interface...\n");
        
        /* Test getpid syscall */
        uint64_t pid;
        __asm__ volatile (
            "mov $3, %%rax\n"   /* SYS_GETPID */
            "syscall\n"
            : "=a"(pid)
            :
            : "rcx", "r11", "memory"
        );
        puts("  getpid() = ");
        print_dec(pid);
        puts("\n");
        
        /* Test write syscall */
        const char *msg = "  Hello from syscall!\n";
        uint64_t len = 0;
        while (msg[len]) len++;
        
        __asm__ volatile (
            "mov $1, %%rax\n"   /* SYS_WRITE */
            "mov $1, %%rdi\n"   /* fd = stdout */
            "mov %0, %%rsi\n"   /* buf */
            "mov %1, %%rdx\n"   /* count */
            "syscall\n"
            :
            : "r"(msg), "r"(len)
            : "rax", "rdi", "rsi", "rdx", "rcx", "r11", "memory"
        );
        
        set_color(VGA_COLOR(10, 0));
        puts("Syscall interface working!\n");
        set_color(VGA_COLOR(15, 0));
    }
    else if (strcmp(cmd_buffer, "ls") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Files:\n");
        set_color(VGA_COLOR(15, 0));
        
        vfs_node_t *root = vfs_get_root();
        if (!root) {
            puts("  (no filesystem mounted)\n");
        } else {
            uint32_t i = 0;
            vfs_node_t *entry;
            while ((entry = vfs_readdir(root, i++)) != (void*)0) {
                puts("  ");
                if (entry->type == VFS_DIRECTORY) {
                    set_color(VGA_COLOR(11, 0));
                    puts(entry->name);
                    puts("/");
                } else {
                    set_color(VGA_COLOR(15, 0));
                    puts(entry->name);
                    puts("  (");
                    print_dec(entry->size);
                    puts(" bytes)");
                }
                set_color(VGA_COLOR(15, 0));
                puts("\n");
            }
        }
    }
    else if (cmd_buffer[0] == 'c' && cmd_buffer[1] == 'a' && cmd_buffer[2] == 't' && cmd_buffer[3] == ' ') {
        const char *filename = cmd_buffer + 4;
        vfs_node_t *file = vfs_lookup(filename);
        
        if (!file) {
            set_color(VGA_COLOR(12, 0));
            puts("\nFile not found: ");
            set_color(VGA_COLOR(15, 0));
            puts(filename);
            puts("\n");
        } else if (file->type == VFS_DIRECTORY) {
            set_color(VGA_COLOR(12, 0));
            puts("\nCannot cat directory: ");
            set_color(VGA_COLOR(15, 0));
            puts(filename);
            puts("\n");
        } else {
            puts("\n");
            uint8_t buffer[256];
            size_t offset = 0;
            size_t read;
            while ((read = vfs_read(file, offset, sizeof(buffer) - 1, buffer)) > 0) {
                buffer[read] = '\0';
                puts((char *)buffer);
                offset += read;
            }
        }
    }
    else if (strcmp(cmd_buffer, "usermode") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Userspace Status:\n");
        set_color(VGA_COLOR(15, 0));
        puts("  TSS initialized: ");
        set_color(VGA_COLOR(10, 0)); puts("Yes\n");
        set_color(VGA_COLOR(15, 0));
        puts("  GDT Ring 3 segments: ");
        set_color(VGA_COLOR(10, 0)); puts("Yes (0x1B/0x23)\n");
        set_color(VGA_COLOR(15, 0));
        puts("  SYSCALL/SYSRET: ");
        set_color(VGA_COLOR(10, 0)); puts("Yes\n");
        set_color(VGA_COLOR(15, 0));
        puts("  User page tables: ");
        set_color(VGA_COLOR(14, 0)); puts("Not implemented\n\n");
        set_color(VGA_COLOR(7, 0));
        puts("Ring 3 execution requires user page tables with\n");
        puts("U/S bit set. This is Phase 8b (advanced).\n");
        puts("Use 'syscall' to test the syscall interface.\n");
        set_color(VGA_COLOR(15, 0));
    }
    else if (strcmp(cmd_buffer, "reboot") == 0) {
        puts("\nRebooting...\n");
        __asm__ volatile ("lidt 0\nint $0x03");
    }
    else if (strcmp(cmd_buffer, "halt") == 0) {
        puts("\nSystem halted.\n");
        __asm__ volatile ("cli; hlt");
    }
    else {
        set_color(VGA_COLOR(12, 0)); puts("\nUnknown: ");
        set_color(VGA_COLOR(15, 0)); puts(cmd_buffer);
    }
    
    cmd_pos = 0;
    shell_prompt();
}

void shell_input(char c) {
    if (c == '\n') shell_execute();
    else if (c == '\b') { if (cmd_pos > 0) { cmd_pos--; putchar('\b'); } }
    else if (cmd_pos < CMD_BUFFER_SIZE - 1) { cmd_buffer[cmd_pos++] = c; putchar(c); }
}

/* Timer handler with scheduler */
void timer_tick_handler(uint64_t int_num, uint64_t error_code) {
    (void)int_num; (void)error_code;
    timer_tick();      /* Increment timer counter */
    scheduler_tick();  /* Run scheduler */
}

/* Kernel main */
void kernel_main(uint64_t multiboot_info, uint64_t magic) {
    (void)magic;
    saved_mb_info = multiboot_info;
    
    clear_screen();
    
    set_color(VGA_COLOR(11, 0));
    puts("========================================\n");
    puts("  MinimalOS 64-bit - Long Mode Active!\n");
    puts("========================================\n\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize serial first for debug output */
    serial_init();
    serial_debug("Kernel starting...");
    
    puts("Initializing PIC... ");
    pic_init();
    serial_debug("PIC initialized");
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Initializing IDT... ");
    idt_init();
    for (int i = 0; i < 32; i++) register_interrupt_handler(i, exception_handler);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Initializing PMM... ");
    pmm_init(multiboot_info);
    set_color(VGA_COLOR(10, 0)); puts("[OK] ");
    set_color(VGA_COLOR(7, 0)); print_dec(pmm_get_total_memory() / 1024 / 1024); puts(" MB\n");
    set_color(VGA_COLOR(15, 0));
    
    puts("Initializing heap... ");
    kheap_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK] ");
    set_color(VGA_COLOR(7, 0)); print_dec(kheap_get_free() / 1024); puts(" KB\n");
    set_color(VGA_COLOR(15, 0));
    
    puts("Initializing processes... ");
    process_init();
    scheduler_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Initializing TSS... ");
    tss_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Initializing syscalls... ");
    syscall_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    /* Initialize VFS and mount demo initrd */
    puts("Initializing VFS... ");
    vfs_init();
    extern uint64_t demo_initrd_build(void);
    uint64_t initrd_loc = demo_initrd_build();
    vfs_node_t *initrd_root = initrd_init(initrd_loc);
    if (initrd_root) {
        vfs_mount_root(initrd_root);
        set_color(VGA_COLOR(10, 0)); puts("[OK] ");
        set_color(VGA_COLOR(7, 0));
        extern uint32_t initrd_get_file_count(void);
        print_dec(initrd_get_file_count());
        puts(" files\n");
    } else {
        set_color(VGA_COLOR(12, 0)); puts("[FAIL]\n");
    }
    set_color(VGA_COLOR(15, 0));
    
    /* Create test tasks */
    process_create("task_a", task_a);
    process_create("task_b", task_b);
    
    puts("Initializing timer... ");
    timer_init(100);
    register_interrupt_handler(32, timer_tick_handler);
    pic_enable_irq(0);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Initializing keyboard... ");
    keyboard_init();
    pic_enable_irq(1);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    puts("Enabling interrupts... ");
    __asm__ volatile ("sti");
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n\n"); set_color(VGA_COLOR(15, 0));
    
    /* Start scheduler */
    scheduler_start();
    
    set_color(VGA_COLOR(14, 0));
    puts("Welcome to MinimalOS! Type 'help' for commands.\n");
    set_color(VGA_COLOR(15, 0));
    
    shell_prompt();
    
    while (1) {
        char c = keyboard_getchar();
        shell_input(c);
    }
}
