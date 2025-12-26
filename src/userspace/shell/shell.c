// Interactive shell for MinimalOS
#define SYS_READ  0
#define SYS_WRITE 1
#define SYS_OPEN  2
#define SYS_CLOSE 3
#define SYS_EXIT  60

#define NULL ((void*)0)

typedef unsigned long u64;
typedef unsigned int u32;
typedef unsigned char u8;
typedef long ssize_t;

// Simple syscall wrapper
static inline u64 syscall3(u64 num, u64 arg1, u64 arg2, u64 arg3) {
    u64 ret;
    __asm__ volatile (
        "mov %1, %%rax\n"
        "mov %2, %%rdi\n"
        "mov %3, %%rsi\n"
        "mov %4, %%rdx\n"
        "syscall"
        : "=a"(ret)
        : "r"(num), "r"(arg1), "r"(arg2), "r"(arg3)
        : "rcx", "r11", "memory"
    );
    return ret;
}

// String functions
static int strlen(const char* str) {
    int len = 0;
    while (str[len]) len++;
    return len;
}

static int strcmp(const char* s1, const char* s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

static void strcpy(char* dest, const char* src) {
    while (*src) {
        *dest++ = *src++;
    }
    *dest = '\0';
}

static void* memset(void* dest, int val, unsigned long len) {
    unsigned char* ptr = dest;
    while (len-- > 0)
        *ptr++ = val;
    return dest;
}

// Output functions
static void print(const char* str) {
    syscall3(SYS_WRITE, 1, (u64)str, strlen(str));
}

// Command buffer
static char cmdbuf[256];
static int cmdlen = 0;

// Built-in: ls - list files (dummy for now)
static void cmd_ls(void) {
    print("bin/   etc/   dev/   tmp/\n");
}

// Built-in: cat - display file contents
static void cmd_cat(const char* path) {
    if (!path || !path[0]) {
        print("Usage: cat <filename>\n");
        return;
    }
    
    // Try to open the file
    int fd = (int)syscall3(SYS_OPEN, (u64)path, 0, 0);
    if (fd < 0) {
        print("cat: cannot open '");
        print(path);
        print("': No such file\n");
        return;
    }
    
    // Read and display contents
    char buffer[512];
    ssize_t bytes_read;
    
    while ((bytes_read = syscall3(SYS_READ, fd, (u64)buffer, sizeof(buffer) - 1)) > 0) {
        buffer[bytes_read] = '\0';
        print(buffer);
    }
    
    syscall3(SYS_CLOSE, fd, 0, 0);
}

// Built-in: pwd - print working directory
static void cmd_pwd(void) {
    print("/\n");
}

// Built-in: help - show available commands
static void cmd_help(void) {
    print("Available commands:\n");
    print("  ls       - List directory contents\n");
    print("  cat FILE - Display file contents\n");
    print("  pwd      - Print working directory\n");
    print("  help     - Show this help message\n");
    print("  exit     - Exit shell\n");
}

// Parse and execute command
static void execute_command(void) {
    if (cmdlen == 0) {
        return;
    }
    
    cmdbuf[cmdlen] = '\0';
    
    // Find first space (if any) to separate command from arguments
    int space_pos = -1;
    for (int i = 0; i < cmdlen; i++) {
        if (cmdbuf[i] == ' ') {
            space_pos = i;
            cmdbuf[i] = '\0';
            break;
        }
    }
    
    const char* cmd = cmdbuf;
    const char* arg = (space_pos >= 0 && space_pos + 1 < cmdlen) ? &cmdbuf[space_pos + 1] : NULL;
    
    // Execute built-in commands
    if (strcmp(cmd, "exit") == 0) {
        print("Goodbye!\n");
        syscall3(SYS_EXIT, 0, 0, 0);
    } else if (strcmp(cmd, "ls") == 0) {
        cmd_ls();
    } else if (strcmp(cmd, "cat") == 0) {
        cmd_cat(arg);
    } else if (strcmp(cmd, "pwd") == 0) {
        cmd_pwd();
    } else if (strcmp(cmd, "help") == 0) {
        cmd_help();
    } else if (cmdlen > 0) {  // Only show error if command not empty
        print("Unknown command: ");
        print(cmd);
        print("\nType 'help' for available commands\n");
    }
}

void _start(void) {
    print("\n");
    print("========================================\n");
    print("MinimalOS Shell v0.1\n");
    print("========================================\n");
    print("Type 'help' for available commands\n");
    print("\n");
    
    while (1) {
        // Print prompt
        print("$ ");
        
        // Read command line from stdin
        cmdlen = 0;
        memset(cmdbuf, 0, sizeof(cmdbuf));
        
        while (1) {
            char c;
            ssize_t n = syscall3(SYS_READ, 0, (u64)&c, 1);
            
            if (n <= 0) {
                break;  // Error or EOF
            }
            
            if (c == '\n') {
                // Echo newline
                syscall3(SYS_WRITE, 1, (u64)"\n", 1);
                break;  // End of line
            } else if (c == '\b' || c == 127) {  // Backspace or DEL
                if (cmdlen > 0) {
                    cmdlen--;
                    // Erase character: backspace, space, backspace
                    syscall3(SYS_WRITE, 1, (u64)"\b \b", 3);
                }
            } else if (c >= 32 && c < 127) {  // Printable ASCII
                if (cmdlen < sizeof(cmdbuf) - 1) {
                    cmdbuf[cmdlen++] = c;
                    // Echo character
                    syscall3(SYS_WRITE, 1, (u64)&c, 1);
                }
            }
        }
        
        // Execute the command
        execute_command();
    }
    
    while(1);  // Hang
}
