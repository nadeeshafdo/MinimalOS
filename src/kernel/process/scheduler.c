#include "scheduler.h"
#include "../lib/printk.h"
#include "../drivers/timer.h"

#define TIME_SLICE_TICKS 10  // 10 ticks = 100ms at 100Hz

static process_t* ready_queue_head = NULL;
static process_t* ready_queue_tail = NULL;
static bool scheduler_enabled = false;

// External context switch function
extern void context_switch(cpu_context_t** old_ctx, cpu_context_t* new_ctx);

static void scheduler_tick(void) {
    if (scheduler_enabled) {
        schedule();
    }
}

void scheduler_init(void) {
    printk("[SCHEDULER] Initializing round-robin scheduler...\n");
    
    ready_queue_head = NULL;
    ready_queue_tail = NULL;
    
    // Register scheduler callback with timer
    timer_register_callback(scheduler_tick);
    
    printk("[SCHEDULER] Time slice: %u ticks (%u ms)\n", 
           TIME_SLICE_TICKS, TIME_SLICE_TICKS * 10);
    printk("[SCHEDULER] Initialization complete!\n");
}

void scheduler_add_process(process_t* proc) {
    if (proc == NULL) {
        return;
    }
    
    proc->state = PROCESS_STATE_READY;
    proc->time_slice = TIME_SLICE_TICKS;
    proc->next = NULL;
    
    if (ready_queue_head == NULL) {
        ready_queue_head = proc;
        ready_queue_tail = proc;
    } else {
        ready_queue_tail->next = proc;
        ready_queue_tail = proc;
    }
    
    printk("[SCHEDULER] Added process '%s' (PID %u) to ready queue\n", 
           proc->name, proc->pid);
}

void scheduler_remove_process(process_t* proc) {
    if (proc == NULL || ready_queue_head == NULL) {
        return;
    }
    
    // Find and remove from queue
    if (ready_queue_head == proc) {
        ready_queue_head = proc->next;
        if (ready_queue_tail == proc) {
            ready_queue_tail = NULL;
        }
    } else {
        process_t* current = ready_queue_head;
        while (current->next != NULL) {
            if (current->next == proc) {
                current->next = proc->next;
                if (ready_queue_tail == proc) {
                    ready_queue_tail = current;
                }
                break;
            }
            current = current->next;
        }
    }
    
    proc->next = NULL;
}

void schedule(void) {
    if (ready_queue_head == NULL) {
        return;  // No processes to schedule
    }
    
    process_t* current = process_get_current();
    
    // If current process still has time, continue running
    if (current != NULL && current->state == PROCESS_STATE_RUNNING) {
        if (current->time_slice > 0) {
            current->time_slice--;
            return;
        }
    }
    
    // Get next process from ready queue
    process_t* next = ready_queue_head;
    if (next == NULL) {
        return;
    }
    
    // Remove from ready queue
    ready_queue_head = next->next;
    if (ready_queue_tail == next) {
        ready_queue_tail = NULL;
    }
    next->next = NULL;
    
    // Add current process back to queue if still ready
    if (current != NULL && current->state == PROCESS_STATE_RUNNING) {
        current->state = PROCESS_STATE_READY;
        scheduler_add_process(current);
    }
    
    // Switch to next process
    next->state = PROCESS_STATE_RUNNING;
    next->time_slice = TIME_SLICE_TICKS;
    
    process_t* old_proc = current;
    process_set_current(next);
    
    // Switch page directory
    if (next->page_directory != NULL) {
        vmm_switch_directory(next->page_directory);
    }
    
    // Perform context switch
    if (old_proc != NULL) {
        context_switch(&old_proc->context, next->context);
    }
}

void yield(void) {
    process_t* current = process_get_current();
    if (current != NULL) {
        current->time_slice = 0;  // Force reschedule
    }
    schedule();
}

void scheduler_enable(void) {
    printk("[SCHEDULER] Enabled\n");
    scheduler_enabled = true;
}

void scheduler_disable(void) {
    scheduler_enabled = false;
    printk("[SCHEDULER] Disabled\n");
}
