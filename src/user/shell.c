// Simple string functions
int strlen(const char *str) {
    int len = 0;
    while (str[len]) len++;
    return len;
}

int strcmp(const char *s1, const char *s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *s1 - *s2;
}

int strncmp(const char *s1, const char *s2, int n) {
    while (n-- && *s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return n < 0 ? 0 : *s1 - *s2;
}

void _user_start() {
    char buf[256];
    int i;
    const char *prompt = "shell> ";
    const char *help_msg = "Commands: echo <text>, help\n";
    const char *unknown_msg = "Unknown command. Type 'help' for available commands.\n";
    const char *newline = "\n";
    
    while (1) {
        i = 0;

        // Syscall for write: prompt
        asm volatile (
            "mov $1, %%rax\n"          // SYS_WRITE
            "syscall"
            :
            : "D"(prompt)              // RDI = prompt
            : "rax"
        );

        // Read input
        while (1) {
            char ch;
            asm volatile (
                "mov $0, %%rax\n"      // SYS_READ
                "syscall"
                : "=a"(ch)             // RAX = return value (char)
                :
                :
            );
            
            if (ch == '\n') break;
            if (i < 255) {
                buf[i++] = ch;
            }
        }
        buf[i] = 0;

        // Simple command: echo
        if (strncmp(buf, "echo ", 5) == 0) {
            asm volatile (
                "mov $1, %%rax\n"
                "syscall"
                :
                : "D"(buf + 5)         // RDI = string pointer
                : "rax"
            );
        } else if (strcmp(buf, "help") == 0) {
            asm volatile (
                "mov $1, %%rax\n"
                "syscall"
                :
                : "D"(help_msg)        // RDI = help message
                : "rax"
            );
        } else if (strcmp(buf, "") != 0) {
            asm volatile (
                "mov $1, %%rax\n"
                "syscall"
                :
                : "D"(unknown_msg)     // RDI = unknown command message
                : "rax"
            );
        }

        // Newline
        asm volatile (
            "mov $1, %%rax\n"
            "syscall"
            :
            : "D"(newline)             // RDI = newline
            : "rax"
        );
    }
}