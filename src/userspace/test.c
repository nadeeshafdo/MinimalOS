#define SYS_WRITE 1
#define SYS_EXIT 60
#define SYS_IPC_SEND 8
#define SYS_IPC_RECV 9

typedef unsigned long u64;
typedef unsigned int u32;
typedef unsigned char u8;

typedef struct {
    u32 sender_pid;
    u32 receiver_pid;
    u32 type;
    u32 length;
    u8 data[1024];
} ipc_message_t;

void syscall(u64 num, u64 arg1, u64 arg2, u64 arg3) {
    __asm__ volatile (
        "mov %0, %%rax\n"
        "mov %1, %%rdi\n"
        "mov %2, %%rsi\n"
        "mov %3, %%rdx\n"
        "syscall"
        : 
        : "r"(num), "r"(arg1), "r"(arg2), "r"(arg3)
        : "rax", "rdi", "rsi", "rdx", "rcx", "r11"
    );
}

void _start(void) {
    syscall(SYS_WRITE, 1, (u64)"Waiting for IPC message...\n", 27);
    
    ipc_message_t msg;
    u32 from_pid = 0;
    
    // This should block until kernel thread sends message
    syscall(SYS_IPC_RECV, (u64)&from_pid, (u64)&msg, 0);
    
    syscall(SYS_WRITE, 1, (u64)"Received Message: ", 18);
    syscall(SYS_WRITE, 1, (u64)msg.data, msg.length);
    syscall(SYS_WRITE, 1, (u64)"\n", 1);
    
    syscall(SYS_EXIT, 0, 0, 0);
    while(1);
}
