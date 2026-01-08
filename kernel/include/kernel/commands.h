#ifndef _KERNEL_COMMANDS_H
#define _KERNEL_COMMANDS_H

#include <stdint.h>

/* Utility functions for commands */
void cmd_print_hex(uint32_t value);
void cmd_print_dec(uint32_t value);
uint32_t cmd_parse_hex(const char *s);
uint32_t cmd_parse_dec(const char *s);
const char* cmd_get_arg(const char *s, char *buf, uint32_t max);

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

/* Memory tool commands */
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

#endif /* _KERNEL_COMMANDS_H */
