# Development Guide

## Adding a Shell Command
The shell is a simple command dispatcher located in `kernel/shell.c`. To add a new command:

1. **Implement the Command**:
   Create a new function in `kernel/commands/` (or an existing file) that implements your command logic.
   ```c
   void cmd_mycommand(const char *args) {
       terminal_writestring("Hello from my command!\n");
   }
   ```

2. **Register the Command**:
   Open `kernel/shell.c` and add your command to the `execute_command` function.
   ```c
   else if (strcmp(cmd_buffer, "mycommand") == 0) cmd_mycommand(NULL);
   ```

3. **Update Headers**:
   Ensure your function prototype is available to `shell.c`, typically by adding it to `kernel/include/kernel/commands.h`.

## Adding a System Call
System calls allow user-space programs (once implemented) to interact with the kernel.

1. **Implement the Handler**:
   In `kernel/process/syscall.c`, create a static function that takes a `struct registers *regs` argument.
   ```c
   static void sys_mycall(struct registers *regs) {
       // Implementation
       // regs->rbx, regs->rcx, etc. contain arguments
   }
   ```

2. **Register in Table**:
   Add your function to the `syscalls[]` array in `kernel/process/syscall.c`. The index in this array will be the system call number.

3. **Usage**:
   The system call can be invoked via interrupt `0x80`, with `RAX` set to the system call number.
