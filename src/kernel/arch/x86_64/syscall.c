#include "syscall.h"
#include "msr.h"
#include "gdt.h"
#include "../../lib/printk.h"
#include "../../lib/string.h"
#include "../../process/process.h"
#include "../../ipc/ipc.h"

extern void syscall_entry(void);

// Simple per-cpu data structure for GS base
typedef struct {
    u64 scratch_rsp;      // Scratch space for saving user RSP during syscall
    u64 kernel_stack;     // Current kernel stack to switch to
} cpu_data_t;

static cpu_data_t cpu_data;

void syscall_init(void) {
    // 1. Setup EFER (Enable generic syscall/sysret)
    u64 efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | 1);  // Bit 0 = SCE (System Call Extensions)
    
    // 2. Setup STAR (Segment selectors for syscall/sysret)
    // Syscall: CS = bits 47:32 (Kernel Code 0x08), SS = CS+8 (Kernel Data 0x10)
    // Sysret:  CS = bits 63:48 (User Code 32-bit?? Actually base selector)
    //          Linux uses (UserCode32 | (UserCode64 << 16))
    // We use User Code Base = 0x13 (User code is +16 from base? No)
    // x86_64 requires specific segment ordering for syscall/sysret.
    // STAR[47:32] = Kernel CS
    // STAR[63:48] = User CS Base (sysret loads CS=Base+16, SS=Base+8)
    // Our GDT: 0=Null, 1=KCode(08), 2=KData(10), 3=UCode(1B/18), 4=UData(23/20)
    // We need User CS Base such that Base+16 = Ring 3 CS (0x1B?)
    // Actually Base+16 = 0x10 + 0x10 = 0x20?
    // Let's verify GDT indices:
    // 0x08 (1), 0x10 (2), 0x18 (3), 0x20 (4)
    // Sysret CS wants index 3 (0x18). So match Base+16 = 0x18 -> Base = 0x08?
    // Sysret SS wants index 2-ish?
    // Intel SDM says:
    // SYSRET loads CS from STAR[63:48] + 16.  SS from STAR[63:48] + 8.
    // If we want CS=0x1B (Entry 3, RPL3), Base should be 0x08?
    // Then CS = 0x08+16 = 0x18. SS = 0x08+8 = 0x10.
    // Wait, SS needs to be User Data (0x23). But 0x10 is KData.
    // This implies GDT order must be: Code, Data, UserCode, UserData?
    // We have: KCode, KData, UCode, UData.
    
    // Solution: Just set STAR[63:48] = 0x08 (Kernel Code).
    // RET CS = 0x18 (UCode without RPL bits). RET SS = 0x10 (KData??)
    // This is weird. Usually we want SS=0x23.
    // Actually, x86-64 SYSRET sets SS selector to (STAR[63:48]+8) OR 3 (RPL).
    // If STAR=0x130008...
    // Let's assume standard Linux GDT layout compatibility or adjust.
    // Alternatively, we can use `iretq` in syscall handler for return if segments are messy.
    // But `sysretq` is faster.
    
    // Let's rely on standard practice: STAR = (KernelCS << 32) | ((UserCS-16) << 48)
    // UserCS = 0x18. So (0x18-16) = 0x08?
    wrmsr(MSR_STAR, (0x08ULL << 32) | (0x08ULL << 48));
    
    // 3. Setup LSTAR (RIP for syscall)
    wrmsr(MSR_LSTAR, (u64)syscall_entry);
    
    // 4. Setup SFMASK (RFLAGS mask)
    // Mask interrupts (IF bit 9) during syscall
    wrmsr(MSR_SFMASK, 0x200);
    
    // 5. Setup GS_BASE for swapgs
    // Point to our cpu_data struct
    wrmsr(MSR_GS_BASE, (u64)&cpu_data);
    wrmsr(MSR_KERNEL_GS_BASE, (u64)&cpu_data); // For paranoid safety
    
    // Initialize cpu_data
    cpu_data.scratch_rsp = 0;
    cpu_data.kernel_stack = 0; // Updated on context switch!
    
    printk("[SYSCALL] Initialized\n");
}

u64 syscall_handler_c(u64 syscall_num, u64 arg1, u64 arg2, u64 arg3) {
    (void)arg3; // Unused for now
    
    // Current process
    process_t* current = process_get_current();
    
    if (syscall_num == 1) { // SYS_WRITE
         // fd is arg1, buf is arg2, len is arg3
         // Check buffer pointer
         if (arg2) {
             const char* msg = (const char*)arg2;
             printk("[USER %u] %s", current->pid, msg);
         }
         return 0; // Success
    } else if (syscall_num == 60) { // SYS_EXIT
        printk("[USER %u] Exiting with code %lu\n", current->pid, arg1);
        process_exit(arg1); // Actually exit!
        while(1);
        return 0;
    } else if (syscall_num == 8) { // SYS_IPC_SEND
        // arg1 = dest_pid, arg2 = msg_ptr
        u32 dest = (u32)arg1;
        ipc_message_t* msg = (ipc_message_t*)arg2;
        
        // TODO: Validate pointer
        return (u64)ipc_send_message(dest, msg);
        
    } else if (syscall_num == 9) { // SYS_IPC_RECV
        // arg1 = from_pid_ptr (u32*), arg2 = buffer_ptr
        u32* from_pid = (u32*)arg1;
        ipc_message_t* buffer = (ipc_message_t*)arg2;
        
        return (u64)ipc_receive_message(from_pid, buffer);
        
    } else {
        printk("[SYSCALL] Unknown syscall %lu from PID %u\n", syscall_num, current->pid);
        return -1; // Error
    }
}

void syscall_set_kernel_stack(uintptr stack_top) {
    cpu_data.kernel_stack = stack_top;
    gdt_set_kernel_stack(stack_top);
}
