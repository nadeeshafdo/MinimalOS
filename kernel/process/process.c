/* Process Management */

#include <stdint.h>
#include "process.h"
#include "kheap.h"

/* Process list */
static process_t *process_list = (void*)0;
static process_t *current_process = (void*)0;
static uint64_t next_pid = 0;
static uint64_t num_processes = 0;

#define KERNEL_STACK_SIZE (16 * 1024)

void process_init(void) {
    process_t *idle = (process_t *)kmalloc(sizeof(process_t));
    if (!idle) return;
    
    idle->pid = next_pid++;
    idle->state = PROCESS_RUNNING;
    idle->name = "kernel";
    idle->stack = (void*)0;
    idle->stack_size = 0;
    idle->rsp = 0;
    idle->rip = 0;
    idle->next = (void*)0;
    
    process_list = idle;
    current_process = idle;
    num_processes = 1;
}

/* Wrapper that calls the entry function and then exits */
static void process_entry_wrapper(void) {
    /* The entry point is stored in r12 by our stack setup */
    void (*entry)(void);
    __asm__ volatile ("mov %%r12, %0" : "=r"(entry));
    
    if (entry) {
        entry();
    }
    
    process_exit();
}

process_t *process_create(const char *name, void (*entry)(void)) {
    process_t *proc = (process_t *)kmalloc(sizeof(process_t));
    if (!proc) return (void*)0;
    
    uint64_t *stack = (uint64_t *)kmalloc(KERNEL_STACK_SIZE);
    if (!stack) {
        kfree(proc);
        return (void*)0;
    }
    
    proc->pid = next_pid++;
    proc->state = PROCESS_READY;
    proc->name = name;
    proc->stack = stack;
    proc->stack_size = KERNEL_STACK_SIZE;
    
    /* Set up stack for switch_context (which pops r15-r12, rbx, rbp then ret) */
    uint64_t *sp = (uint64_t *)((uint64_t)stack + KERNEL_STACK_SIZE);
    
    /* Return address (where 'ret' will jump to) */
    *--sp = (uint64_t)process_entry_wrapper;
    
    /* Callee-saved registers that switch_context will pop */
    *--sp = 0;                  /* RBP */
    *--sp = 0;                  /* RBX */
    *--sp = (uint64_t)entry;    /* R12 - we use this to pass entry point */
    *--sp = 0;                  /* R13 */
    *--sp = 0;                  /* R14 */
    *--sp = 0;                  /* R15 */
    
    proc->rsp = (uint64_t)sp;
    proc->rip = (uint64_t)entry;
    
    /* Add to process list */
    proc->next = (void*)0;
    process_t *tail = process_list;
    while (tail->next) tail = tail->next;
    tail->next = proc;
    
    num_processes++;
    
    return proc;
}

process_t *process_current(void) {
    return current_process;
}

process_t *process_get(uint64_t pid) {
    process_t *proc = process_list;
    while (proc) {
        if (proc->pid == pid) return proc;
        proc = proc->next;
    }
    return (void*)0;
}

uint64_t process_count(void) {
    return num_processes;
}

extern void switch_context(uint64_t *old_rsp, uint64_t new_rsp);

void schedule(void) {
    if (!current_process || num_processes <= 1) return;
    
    process_t *next = current_process->next;
    if (!next) next = process_list;
    
    process_t *start = next;
    while (next->state != PROCESS_READY && next->state != PROCESS_RUNNING) {
        next = next->next;
        if (!next) next = process_list;
        if (next == start) return;
    }
    
    if (next == current_process) return;
    
    process_t *old = current_process;
    if (old->state == PROCESS_RUNNING) {
        old->state = PROCESS_READY;
    }
    
    current_process = next;
    next->state = PROCESS_RUNNING;
    
    switch_context(&old->rsp, next->rsp);
}

void process_yield(void) {
    schedule();
}

void process_exit(void) {
    if (!current_process || current_process->pid == 0) return;
    
    current_process->state = PROCESS_TERMINATED;
    num_processes--;
    
    schedule();
    
    while (1) __asm__ volatile ("hlt");
}
