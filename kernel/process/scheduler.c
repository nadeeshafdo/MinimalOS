#include <stdint.h>
#include <stddef.h>
#include <kernel/scheduler.h>
#include <kernel/process.h>
#include <kernel/tty.h>

/* Interrupt control */
static inline void cli(void) { __asm__ volatile("cli"); }
static inline void sti(void) { __asm__ volatile("sti"); }

/* Ready queue (simple linked list) */
static process_t *ready_queue_head = NULL;
static process_t *ready_queue_tail = NULL;

/* Scheduling enabled flag */
static int scheduling_enabled = 0;

void scheduler_init(void) {
    ready_queue_head = NULL;
    ready_queue_tail = NULL;
    scheduling_enabled = 0;
}

void scheduler_add(process_t *proc) {
    if (!proc) return;
    
    proc->next = NULL;
    
    if (!ready_queue_tail) {
        ready_queue_head = proc;
        ready_queue_tail = proc;
    } else {
        ready_queue_tail->next = proc;
        ready_queue_tail = proc;
    }
}

void scheduler_remove(process_t *proc) {
    if (!proc || !ready_queue_head) return;
    
    /* Handle head removal */
    if (ready_queue_head == proc) {
        ready_queue_head = proc->next;
        if (!ready_queue_head) {
            ready_queue_tail = NULL;
        }
        proc->next = NULL;
        return;
    }
    
    /* Search for process */
    process_t *prev = ready_queue_head;
    while (prev->next && prev->next != proc) {
        prev = prev->next;
    }
    
    if (prev->next == proc) {
        prev->next = proc->next;
        if (ready_queue_tail == proc) {
            ready_queue_tail = prev;
        }
        proc->next = NULL;
    }
}

process_t *scheduler_next(void) {
    if (!ready_queue_head) {
        return NULL;
    }
    
    /* Find first runnable process */
    process_t *next = NULL;
    process_t *current = ready_queue_head;
    process_t *prev = NULL;
    
    while (current) {
        if (current->state == PROCESS_STATE_READY) {
            /* Found a ready process - remove it from queue */
            if (prev) {
                prev->next = current->next;
            } else {
                ready_queue_head = current->next;
            }
            
            if (ready_queue_tail == current) {
                ready_queue_tail = prev;
            }
            
            next = current;
            next->next = NULL;
            break;
        }
        
        /* Skip non-ready processes (zombie, blocked, etc.) */
        prev = current;
        current = current->next;
    }
    
    return next;
}

void scheduler_tick(void) {
    if (!scheduling_enabled) return;
    
    process_t *current = process_current();
    if (!current) return;
    
    /* Decrement time slice */
    if (current->time_slice > 0) {
        current->time_slice--;
    }
    
    /* Time slice expired - switch to next process */
    if (current->time_slice == 0) {
        current->time_slice = DEFAULT_TIME_SLICE;
        
        /* Disable interrupts during scheduling decision */
        cli();
        
        process_t *next = scheduler_next();
        if (next && next != current) {
            /* Add current to ready queue if still runnable */
            if (current->state == PROCESS_STATE_RUNNING) {
                current->state = PROCESS_STATE_READY;
                scheduler_add(current);
            }
            
            /* Switch to next process */
            extern void process_switch(process_t *next);
            extern void tss_set_stack(uint32_t esp0);
            
            if (next->kernel_stack) {
                tss_set_stack(next->kernel_stack);
            }
            process_switch(next);
            /* Interrupts re-enabled when we return here after being scheduled back */
        } else {
            sti();  /* Re-enable if no switch */
        }
    }
}

void scheduler_start(void) {
    scheduling_enabled = 1;
    
    /* Get first process */
    process_t *first = scheduler_next();
    if (first) {
        extern void process_switch(process_t *next);
        extern void tss_set_stack(uint32_t esp0);
        
        if (first->kernel_stack) {
            tss_set_stack(first->kernel_stack);
        }
        process_switch(first);
    }
}
