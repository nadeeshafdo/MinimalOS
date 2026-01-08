/* Shell commands header */
#ifndef KERNEL_COMMANDS_H
#define KERNEL_COMMANDS_H

#include <kernel/tty.h>
#include <stdint.h>

/* Basic commands */
void cmd_help(void);
void cmd_clear(void);
void cmd_echo(const char *args);
void cmd_reboot(void);
void cmd_halt(void);
void cmd_poweroff(void);

/* System info commands */
void cmd_info(void);
void cmd_mem(void);
void cmd_uptime(void);
void cmd_ps(void);
void cmd_cpuid(void);

/* Memory commands */
void cmd_peek(const char *args);
void cmd_poke(const char *args);
void cmd_hexdump(const char *args);
void cmd_alloc(const char *args);

/* Display commands */
void cmd_color(const char *args);
void cmd_banner(void);

/* Test commands */
void cmd_test(void);
void cmd_panic(void);
void cmd_cpufreq(void);

/* Utility functions */
uint64_t parse_hex(const char *str);
uint64_t parse_dec(const char *str);
void print_hex64(uint64_t value);
void print_dec64(uint64_t value);

#endif
