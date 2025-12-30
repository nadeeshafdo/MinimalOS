/**
 * MinimalOS - Unified Boot Information Structure
 * Provides consistent interface for both BIOS (Multiboot2) and UEFI boot
 */

#ifndef MINIMALOS_BOOTINFO_H
#define MINIMALOS_BOOTINFO_H

#include <minimalos/types.h>

/* Boot type */
typedef enum {
  BOOT_TYPE_MULTIBOOT2,
  BOOT_TYPE_UEFI,
} boot_type_t;

/* Memory region types */
typedef enum {
  MEMORY_AVAILABLE = 1,
  MEMORY_RESERVED = 2,
  MEMORY_ACPI_RECLAIMABLE = 3,
  MEMORY_ACPI_NVS = 4,
  MEMORY_BAD = 5,
} memory_type_t;

/* Memory map entry */
struct memory_region {
  uint64_t base;
  uint64_t length;
  uint32_t type;
  uint32_t reserved;
} __packed;

/* Framebuffer info */
struct framebuffer_info {
  uint64_t addr;
  uint32_t pitch;
  uint32_t width;
  uint32_t height;
  uint8_t bpp;
  uint8_t type;
} __packed;

/* Unified boot info structure */
struct boot_info {
  uint32_t magic;
  boot_type_t boot_type;

  /* Memory map */
  uint32_t memory_map_entries;
  struct memory_region *memory_map;

  /* Framebuffer */
  struct framebuffer_info framebuffer;

  /* ACPI */
  uint64_t rsdp_addr;

  /* Kernel location */
  uint64_t kernel_phys_start;
  uint64_t kernel_phys_end;

  /* Command line */
  char *cmdline;
};

#define BOOTINFO_MAGIC 0x4D494E4F /* "MINO" */

#endif /* MINIMALOS_BOOTINFO_H */
