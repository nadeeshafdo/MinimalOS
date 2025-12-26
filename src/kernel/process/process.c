#include "process.h"
#include "../mm/pmm.h"
#include "../mm/heap.h"
#include "../lib/string.h"
#include "../lib/printk.h"
#include "scheduler.h"

static process_t* process_table[MAX_PROCESSES];
static u32 next_pid = 1;
static process_t* current_process = NULL;

void process_init(void) {
    printk("[PROCESS] Initializing process management...\n");
    
    // Clear process table
    for (u32 i = 0; i < MAX_PROCESSES; i++) {
        process_table[i] = NULL;
    }
    
    // Create kernel idle process (PID 0)
    process_t* idle = process_create("idle");
    if (idle) {
        idle->pid = 0;
        idle->state = PROCESS_STATE_RUNNING;
        current_process = idle;
        printk("[PROCESS] Created idle process (PID 0)\n");
    }
    
    printk("[PROCESS] Initialization complete!\n");
}

process_t* process_create(const char* name) {
    // Find free PID
    u32 pid = 0;
    for (u32 i = 0; i < MAX_PROCESSES; i++) {
        if (process_table[i] == NULL) {
            pid = i;
            break;
        }
    }
    
    if (pid == 0 && process_table[0] != NULL) {
        printk("[PROCESS] ERROR: No free PID slots!\n");
        return NULL;
    }
    
    // Allocate PCB
    process_t* proc = (process_t*)kzalloc(sizeof(process_t));
    if (proc == NULL) {
        printk("[PROCESS] ERROR: Failed to allocate PCB!\n");
        return NULL;
    }
    
    // Initialize PCB
    proc->pid = (pid == 0) ? 0 : next_pid++;
    strncpy(proc->name, name, sizeof(proc->name) - 1);
    proc->state = PROCESS_STATE_CREATED;
    proc->parent = NULL;
    proc->next = NULL;
    proc->exit_code = 0;
    
    // Initialize IPC mailbox
    proc->mailbox_head = 0;
    proc->mailbox_tail = 0;
    proc->mailbox_count = 0;
    proc->blocked_on_receive = 0;
    
    // Initialize file descriptors as NULL0;
    proc->priority = 0;
    proc->time_slice = 0;
    
    // Allocate kernel stack
    proc->kernel_stack = (uintptr)kmalloc(KERNEL_STACK_SIZE);
    if (proc->kernel_stack == 0) {
        printk("[PROCESS] ERROR: Failed to allocate kernel stack!\n");
        kfree(proc);
        return NULL;
    }
    
    // Allocate user stack (will be mapped later)
    proc->user_stack = 0;
    
    // Create address space
    proc->page_directory = vmm_create_address_space();
    if (proc->page_directory == NULL) {
        printk("[PROCESS] ERROR: Failed to create address space!\n");
        kfree((void*)proc->kernel_stack);
        kfree(proc);
        return NULL;
    }
    
    // Allocate context structure
    proc->context = (cpu_context_t*)kzalloc(sizeof(cpu_context_t));
    if (proc->context == NULL) {
        printk("[PROCESS] ERROR: Failed to allocate context!\n");
        vmm_destroy_address_space(proc->page_directory);
        kfree((void*)proc->kernel_stack);
        kfree(proc);
        return NULL;
    }
    
    // Add to process table
    process_table[proc->pid] = proc;
    
    printk("[PROCESS] Created process '%s' (PID %u)\n", proc->name, proc->pid);
    
    return proc;
}

void process_setup_kernel_thread(process_t* proc, void (*entry_point)(void)) {
    if (!proc || !entry_point) {
        return;
    }
    
    // Allocate kernel stack (1 page = 4KB)
    uintptr stack_base = pmm_alloc_frame();
    if (stack_base == 0) {
        printk("[PROC] ERROR: Failed to allocate stack for process %u\n", proc->pid);
        return;
    }
    
    // Stack grows downward, so stack pointer starts at top
    uintptr stack_top = stack_base + PAGE_SIZE;
    
    // Push entry point address onto stack (for context_switch to "return" to)
    u64* stack_ptr = (u64*)(stack_top - 8);
    *stack_ptr = (u64)entry_point;
    
    // Initialize CPU context for kernel thread
    // Zero all registers for clean state
    memset(proc->context, 0, sizeof(cpu_context_t));
    
    // Set instruction pointer to entry point (saved as "return address")
    proc->context->rip = (u64)entry_point;
    
    // Set stack pointer (below the pushed entry address)
    proc->context->rsp = (u64)stack_ptr;
    
    // Set callee-saved registers to known state
    proc->context->rbp = 0;  // Frame pointer (null for initial frame)
    proc->context->rflags = 0x202; // IF=1, Reserved=1
    proc->context->cs = 0x08;      // Kernel Code
    proc->context->ss = 0x10;      // Kernel Data
    
    // Set RFLAGS with interrupts enabled (bit 9 = IF)
    proc->context->rflags = 0x202;
    
    // Kernel threads run in ring 0, use kernel segments
    proc->context->cs = 0x08;  // Kernel code segment
    proc->context->ss = 0x10;  // Kernel data segment
    
    // Mark as ready to run
    process_set_state(proc, PROCESS_STATE_READY);
    
    printk("[PROC] Setup kernel thread '%s' (PID %u) at entry %p, stack %p\n",
           proc->name, proc->pid, entry_point, (void*)stack_top);
}

void process_destroy(process_t* proc) {
    if (proc == NULL) {
        return;
    }
    
    printk("[PROCESS] Destroying process '%s' (PID %u)\n", proc->name, proc->pid);
    
    // Remove from process table
    if (proc->pid < MAX_PROCESSES) {
        process_table[proc->pid] = NULL;
    }
    
    // Free resources
    if (proc->context) {
        kfree(proc->context);
    }
    
    if (proc->kernel_stack) {
        kfree((void*)proc->kernel_stack);
    }
    
    if (proc->page_directory) {
        vmm_destroy_address_space(proc->page_directory);
    }
    
    kfree(proc);
}

process_t* process_get_current(void) {
    return current_process;
}



void process_set_current(process_t* proc) {
    current_process = proc;
}

void process_exit(int code) {
    if (current_process == NULL) {
        return;
    }
    
    printk("[PROCESS] PID %u exiting with code %d\n", current_process->pid, code);
    
    current_process->state = PROCESS_STATE_ZOMBIE;
    current_process->exit_code = code;
    
    // We should notify parent here, but for now just yield forever
    // The idle process or a reaper would free it.
    // For now we leak memory to be safe.
    
    while(1) {
        yield();
    }
}


process_t* process_get_by_pid(u32 pid) {
    if (pid >= MAX_PROCESSES) {
        return NULL;
    }
    return process_table[pid];
}

void process_set_state(process_t* proc, process_state_t state) {
    if (proc) {
        proc->state = state;
    }
}

process_state_t process_get_state(process_t* proc) {
    return proc ? proc->state : PROCESS_STATE_DEAD;
}
