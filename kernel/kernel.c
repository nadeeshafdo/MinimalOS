/* MinimalOS Kernel - x86_64 with Limine bootloader
 *
 * This kernel uses the Limine boot protocol for BIOS/UEFI boot support.
 */

#include <kernel/framebuffer.h>
#include <kernel/gdt.h>
#include <kernel/idt.h>
#include <kernel/irq.h>
#include <kernel/isr.h>
#include <kernel/keyboard.h>
#include <kernel/kheap.h>
#include <kernel/paging.h>
#include <kernel/pmm.h>
#include <kernel/process.h>
#include <kernel/scheduler.h>
#include <kernel/shell.h>
#include <kernel/syscall.h>
#include <kernel/timer.h>
#include <kernel/tty.h>
#include <limine.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* ==================== Limine Requests ==================== */

/* Base revision - required for Limine protocol */
__attribute__((
    used, section(".limine_requests"))) static volatile LIMINE_BASE_REVISION(3)

    /* Request start marker */
    __attribute__((
        used, section(".limine_requests_"
                      "start"))) static volatile LIMINE_REQUESTS_START_MARKER

    /* Framebuffer request */
    __attribute__((used, section(".limine_requests"))) static volatile struct
    limine_framebuffer_request framebuffer_request = {
        .id = LIMINE_FRAMEBUFFER_REQUEST, .revision = 0};

/* Memory map request */
__attribute__((
    used,
    section(".limine_requests"))) static volatile struct limine_memmap_request
    memmap_request = {.id = LIMINE_MEMMAP_REQUEST, .revision = 0};

/* HHDM (Higher Half Direct Map) request */
__attribute__((
    used,
    section(".limine_requests"))) static volatile struct limine_hhdm_request
    hhdm_request = {.id = LIMINE_HHDM_REQUEST, .revision = 0};

/* Bootloader info request */
__attribute__((used, section(".limine_requests"))) static volatile struct
    limine_bootloader_info_request bootloader_info_request = {
        .id = LIMINE_BOOTLOADER_INFO_REQUEST, .revision = 0};

/* Firmware type request */
__attribute__((used, section(".limine_requests"))) static volatile struct
    limine_firmware_type_request firmware_type_request = {
        .id = LIMINE_FIRMWARE_TYPE_REQUEST, .revision = 0};

/* Request end marker */
__attribute__((
    used,
    section(".limine_requests_end"))) static volatile LIMINE_REQUESTS_END_MARKER

    /* ==================== Required C library functions ==================== */

    /* GCC/Clang may generate calls to these even if not used directly */
    void *
    memcpy(void *restrict dest, const void *restrict src, size_t n) {
  uint8_t *restrict pdest = (uint8_t *restrict)dest;
  const uint8_t *restrict psrc = (const uint8_t *restrict)src;
  for (size_t i = 0; i < n; i++) {
    pdest[i] = psrc[i];
  }
  return dest;
}

void *memset(void *s, int c, size_t n) {
  uint8_t *p = (uint8_t *)s;
  for (size_t i = 0; i < n; i++) {
    p[i] = (uint8_t)c;
  }
  return s;
}

void *memmove(void *dest, const void *src, size_t n) {
  uint8_t *pdest = (uint8_t *)dest;
  const uint8_t *psrc = (const uint8_t *)src;
  if (src > dest) {
    for (size_t i = 0; i < n; i++) {
      pdest[i] = psrc[i];
    }
  } else if (src < dest) {
    for (size_t i = n; i > 0; i--) {
      pdest[i - 1] = psrc[i - 1];
    }
  }
  return dest;
}

int memcmp(const void *s1, const void *s2, size_t n) {
  const uint8_t *p1 = (const uint8_t *)s1;
  const uint8_t *p2 = (const uint8_t *)s2;
  for (size_t i = 0; i < n; i++) {
    if (p1[i] != p2[i]) {
      return p1[i] < p2[i] ? -1 : 1;
    }
  }
  return 0;
}

/* ==================== Global state ==================== */

/* HHDM offset for physical-to-virtual conversion */
static uint64_t hhdm_offset = 0;

uint64_t get_hhdm_offset(void) { return hhdm_offset; }

/* Convert physical address to virtual using HHDM */
void *phys_to_virt(uint64_t phys) { return (void *)(phys + hhdm_offset); }

/* Convert virtual address to physical */
uint64_t virt_to_phys(void *virt) { return (uint64_t)virt - hhdm_offset; }

/* ==================== Helper functions ==================== */

/* Halt and catch fire */
static void hcf(void) {
  __asm__ volatile("cli");
  for (;;) {
    __asm__ volatile("hlt");
  }
}

/* Kernel panic function */
void kernel_panic(const char *message) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
  terminal_writestring("\n\n*** KERNEL PANIC ***\n");
  terminal_writestring(message);
  terminal_writestring("\nSystem halted.\n");
  hcf();
}

/* Print a number in hexadecimal */
static void print_hex(uint64_t value) {
  char hex[19] = "0x0000000000000000";
  const char *digits = "0123456789ABCDEF";

  for (int i = 17; i >= 2; i--) {
    hex[i] = digits[value & 0xF];
    value >>= 4;
  }

  terminal_writestring(hex);
}

/* Print a number in decimal */
static void print_dec(uint64_t value) {
  char buf[21];
  int i = 20;
  buf[i] = '\0';

  if (value == 0) {
    terminal_writestring("0");
    return;
  }

  while (value > 0 && i > 0) {
    buf[--i] = '0' + (value % 10);
    value /= 10;
  }

  terminal_writestring(&buf[i]);
}

/* Shell task - idles and lets keyboard interrupts handle input */
void shell_task(void) {
  while (1) {
    __asm__ volatile("hlt");
  }
}

/* ==================== Kernel Main Entry Point ==================== */

void kmain(void) {
  /* Ensure the bootloader understood our base revision */
  if (LIMINE_BASE_REVISION_SUPPORTED == false) {
    hcf();
  }

  /* Get HHDM offset (required for physical memory access) */
  if (hhdm_request.response == NULL) {
    hcf();
  }
  hhdm_offset = hhdm_request.response->offset;

  /* Get framebuffer */
  int have_framebuffer = 0;
  if (framebuffer_request.response != NULL &&
      framebuffer_request.response->framebuffer_count > 0) {
    struct limine_framebuffer *fb =
        framebuffer_request.response->framebuffers[0];
    have_framebuffer = fb_init_limine(fb);
  }

  /* Initialize terminal with framebuffer if available */
  if (have_framebuffer) {
    terminal_set_framebuffer(1);
  }
  terminal_initialize();

  /* Display welcome banner */
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("======================================\n");
  terminal_writestring("       MinimalOS v0.2 (x86_64)\n");
  terminal_writestring("======================================\n\n");

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Show bootloader info */
  if (bootloader_info_request.response != NULL) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring(" Booted by: ");
    terminal_writestring(bootloader_info_request.response->name);
    terminal_writestring(" ");
    terminal_writestring(bootloader_info_request.response->version);
    terminal_writestring("\n");
  }

  /* Show firmware type */
  if (firmware_type_request.response != NULL) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring(" Firmware: ");
    switch (firmware_type_request.response->firmware_type) {
    case LIMINE_FIRMWARE_TYPE_X86BIOS:
      terminal_writestring("BIOS");
      break;
    case LIMINE_FIRMWARE_TYPE_UEFI32:
      terminal_writestring("UEFI (32-bit)");
      break;
    case LIMINE_FIRMWARE_TYPE_UEFI64:
      terminal_writestring("UEFI (64-bit)");
      break;
    default:
      terminal_writestring("Unknown");
    }
    terminal_writestring("\n");
  }

  /* Show HHDM offset */
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
  terminal_writestring(" HHDM offset: ");
  print_hex(hhdm_offset);
  terminal_writestring("\n");

  /* Show framebuffer info */
  if (have_framebuffer) {
    framebuffer_info_t *fb = fb_get_info();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring(" Framebuffer: ");
    print_dec(fb->width);
    terminal_writestring("x");
    print_dec(fb->height);
    terminal_writestring("x");
    print_dec(fb->bpp);
    terminal_writestring("\n");
  }

  /* Initialize GDT */
  terminal_writestring("Initializing GDT... ");
  gdt_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize IDT */
  terminal_writestring("Initializing IDT... ");
  idt_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize ISRs */
  terminal_writestring("Initializing ISRs... ");
  isr_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize IRQs */
  terminal_writestring("Initializing IRQs and PIC... ");
  irq_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize timer (100 Hz) */
  terminal_writestring("Initializing timer (100 Hz)... ");
  timer_init(100);
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize keyboard */
  terminal_writestring("Initializing keyboard... ");
  keyboard_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize physical memory manager using Limine's memory map */
  terminal_writestring("Initializing PMM... ");
  if (memmap_request.response != NULL) {
    pmm_init_limine(memmap_request.response);
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("     Free memory: ");
    print_dec(pmm_get_free_memory() / 1024 / 1024);
    terminal_writestring(" MB\n");
  } else {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
    terminal_writestring("[SKIP] No memory map\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
  }

  /* Initialize kernel heap */
  terminal_writestring("Initializing kernel heap... ");
  kheap_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize process management */
  terminal_writestring("Initializing process management... ");
  process_init();
  scheduler_init();
  syscall_init();
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
  terminal_writestring("[OK]\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  terminal_writestring("\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
  terminal_writestring("*** System Initialization Complete ***\n\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
  terminal_writestring("Welcome to MinimalOS (64-bit)!\n");
  terminal_writestring(
      "Built with Limine bootloader for BIOS/UEFI support.\n\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("Features:\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
  terminal_writestring("  * 64-bit Long Mode kernel\n");
  terminal_writestring("  * BIOS and UEFI boot support\n");
  terminal_writestring("  * Framebuffer graphics (Limine)\n");
  terminal_writestring("  * GDT with kernel/user segments\n");
  terminal_writestring("  * IDT with CPU exception handlers\n");
  terminal_writestring("  * Hardware interrupts (PIC)\n");
  terminal_writestring("  * PS/2 keyboard driver\n");
  terminal_writestring("  * Physical memory manager\n");
  terminal_writestring("  * Kernel heap (kmalloc/kfree)\n");

  terminal_writestring("\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
  terminal_writestring("Type 'help' for available commands.\n\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Initialize and start shell */
  shell_init();

  /* Create shell task */
  process_t *shell = process_create("Shell", shell_run);
  scheduler_add(shell);

  /* Start scheduler */
  scheduler_start();

  /* Should not be reached */
  hcf();
}
