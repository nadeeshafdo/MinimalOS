/* Process management for x86_64 */
#include <kernel/kheap.h>
#include <kernel/pmm.h>
#include <kernel/process.h>
#include <kernel/tty.h>
#include <stddef.h>
#include <stdint.h>

/* Process table */
static process_t *processes[MAX_PROCESSES];
static process_t *current_process = NULL;
static uint32_t next_pid = 1;

/* String copy function */
static void strcpy_s(char *dest, const char *src, size_t max) {
  size_t i = 0;
  while (src[i] && i < max - 1) {
    dest[i] = src[i];
    i++;
  }
  dest[i] = '\0';
}

/* External context switch function (assembly) */
extern void context_switch(cpu_context_t *old, cpu_context_t *new);

void process_init(void) {
  /* Clear process table */
  for (int i = 0; i < MAX_PROCESSES; i++) {
    processes[i] = NULL;
  }

  /* Create idle/kernel process (PID 0) */
  process_t *kernel_proc = (process_t *)kmalloc(sizeof(process_t));
  if (!kernel_proc) {
    terminal_writestring("Failed to create kernel process!\n");
    return;
  }

  kernel_proc->pid = 0;
  kernel_proc->state = PROCESS_STATE_RUNNING;
  strcpy_s(kernel_proc->name, "kernel", 32);
  kernel_proc->page_dir = paging_get_directory();
  kernel_proc->kernel_stack = 0; /* Using boot stack */
  kernel_proc->priority = 0;
  kernel_proc->time_slice = 0; /* Kernel doesn't get preempted like this */
  kernel_proc->next = NULL;

  processes[0] = kernel_proc;
  current_process = kernel_proc;
}

process_t *process_create(const char *name, void (*entry)(void)) {
  /* Find free PID */
  uint32_t pid = next_pid++;
  if (pid >= MAX_PROCESSES) {
    return NULL;
  }

  /* Allocate PCB */
  process_t *proc = (process_t *)kmalloc(sizeof(process_t));
  if (!proc) {
    return NULL;
  }

  /* Allocate kernel stack (8KB for 64-bit) */
  uint64_t stack = (uint64_t)kmalloc(8192);
  if (!stack) {
    kfree(proc);
    return NULL;
  }

  /* Initialize PCB */
  proc->pid = pid;
  proc->state = PROCESS_STATE_READY;
  strcpy_s(proc->name, name, 32);
  proc->page_dir =
      paging_get_directory();        /* Share kernel page directory for now */
  proc->kernel_stack = stack + 8192; /* Stack grows downward */
  proc->priority = 1;
  proc->time_slice = 10;
  proc->next = NULL;

  /* Set up initial context */
  proc->context.rip = (uint64_t)entry;
  proc->context.rsp = proc->kernel_stack;
  proc->context.rbp = proc->kernel_stack;
  proc->context.rflags = 0x202; /* Interrupts enabled */
  proc->context.r15 = 0;
  proc->context.r14 = 0;
  proc->context.r13 = 0;
  proc->context.r12 = 0;
  proc->context.rbx = 0;

  /* Add to process table */
  processes[pid] = proc;

  return proc;
}

void process_exit(int status) {
  (void)status;

  if (!current_process || current_process->pid == 0) {
    return; /* Can't exit kernel process */
  }

  /* Mark as zombie - cleanup will happen later */
  current_process->state = PROCESS_STATE_ZOMBIE;

  /* Free kernel stack */
  if (current_process->kernel_stack) {
    kfree((void *)(current_process->kernel_stack - 8192));
  }

  /* Remove from process table */
  processes[current_process->pid] = NULL;

  /* Free PCB */
  process_t *proc_to_free = current_process;

  /* Trigger reschedule */
  extern void scheduler_tick(void);
  current_process->time_slice = 0;
  scheduler_tick();

  /* Should not reach here */
  kfree(proc_to_free);
  while (1) {
    __asm__ volatile("hlt");
  }
}

process_t *process_current(void) { return current_process; }

process_t *process_get(uint32_t pid) {
  if (pid >= MAX_PROCESSES) {
    return NULL;
  }
  return processes[pid];
}

void process_yield(void) { /* Placeholder - scheduler handles actual switch */ }

/* Called by scheduler to switch to a process */
void process_switch(process_t *next) {
  if (!next || next == current_process) {
    return;
  }

  process_t *prev = current_process;
  current_process = next;

  prev->state = PROCESS_STATE_READY;
  next->state = PROCESS_STATE_RUNNING;

  /* Switch page directory if different */
  if (prev->page_dir != next->page_dir) {
    paging_switch_directory(next->page_dir);
  }

  /* Perform context switch */
  context_switch(&prev->context, &next->context);
}
