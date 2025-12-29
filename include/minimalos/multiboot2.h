/**
 * MinimalOS - Multiboot2 Structures
 * Based on Multiboot2 Specification
 */

#ifndef MINIMALOS_MULTIBOOT2_H
#define MINIMALOS_MULTIBOOT2_H

#include <minimalos/types.h>

/* Multiboot2 magic values */
#define MULTIBOOT2_MAGIC 0xE85250D6
#define MULTIBOOT2_BOOTLOADER_MAGIC 0x36D76289

/* Multiboot2 header tags */
#define MULTIBOOT2_TAG_END 0
#define MULTIBOOT2_TAG_CMDLINE 1
#define MULTIBOOT2_TAG_BOOTLOADER_NAME 2
#define MULTIBOOT2_TAG_MODULE 3
#define MULTIBOOT2_TAG_BASIC_MEMINFO 4
#define MULTIBOOT2_TAG_BOOTDEV 5
#define MULTIBOOT2_TAG_MMAP 6
#define MULTIBOOT2_TAG_VBE 7
#define MULTIBOOT2_TAG_FRAMEBUFFER 8
#define MULTIBOOT2_TAG_ELF_SECTIONS 9
#define MULTIBOOT2_TAG_APM 10
#define MULTIBOOT2_TAG_EFI32 11
#define MULTIBOOT2_TAG_EFI64 12
#define MULTIBOOT2_TAG_SMBIOS 13
#define MULTIBOOT2_TAG_ACPI_OLD 14
#define MULTIBOOT2_TAG_ACPI_NEW 15
#define MULTIBOOT2_TAG_NETWORK 16
#define MULTIBOOT2_TAG_EFI_MMAP 17
#define MULTIBOOT2_TAG_EFI_BS 18
#define MULTIBOOT2_TAG_EFI32_IH 19
#define MULTIBOOT2_TAG_EFI64_IH 20
#define MULTIBOOT2_TAG_LOAD_BASE_ADDR 21

/* Memory map entry types */
#define MULTIBOOT2_MEMORY_AVAILABLE 1
#define MULTIBOOT2_MEMORY_RESERVED 2
#define MULTIBOOT2_MEMORY_ACPI_RECLAIMABLE 3
#define MULTIBOOT2_MEMORY_NVS 4
#define MULTIBOOT2_MEMORY_BADRAM 5

/* Multiboot2 information header */
struct __attribute__((packed)) multiboot2_info {
  uint32_t total_size;
  uint32_t reserved;
  /* Tags follow */
};

/* Generic tag header */
struct __attribute__((packed)) multiboot2_tag {
  uint32_t type;
  uint32_t size;
};

/* Command line tag */
struct __attribute__((packed)) multiboot2_tag_string {
  uint32_t type;
  uint32_t size;
  char string[];
};

/* Basic memory info tag */
struct __attribute__((packed)) multiboot2_tag_basic_meminfo {
  uint32_t type;
  uint32_t size;
  uint32_t mem_lower; /* KB below 1MB */
  uint32_t mem_upper; /* KB above 1MB */
};

/* Memory map entry */
struct __attribute__((packed)) multiboot2_mmap_entry {
  uint64_t addr;
  uint64_t len;
  uint32_t type;
  uint32_t reserved;
};

/* Memory map tag */
struct __attribute__((packed)) multiboot2_tag_mmap {
  uint32_t type;
  uint32_t size;
  uint32_t entry_size;
  uint32_t entry_version;
  struct multiboot2_mmap_entry entries[];
};

/* Module tag */
struct __attribute__((packed)) multiboot2_tag_module {
  uint32_t type;
  uint32_t size;
  uint32_t mod_start;
  uint32_t mod_end;
  char cmdline[];
};

/* ACPI RSDP tag (old - v1.0) */
struct __attribute__((packed)) multiboot2_tag_old_acpi {
  uint32_t type;
  uint32_t size;
  uint8_t rsdp[];
};

/* ACPI RSDP tag (new - v2.0+) */
struct __attribute__((packed)) multiboot2_tag_new_acpi {
  uint32_t type;
  uint32_t size;
  uint8_t rsdp[];
};

/* Framebuffer tag */
struct __attribute__((packed)) multiboot2_tag_framebuffer {
  uint32_t type;
  uint32_t size;
  uint64_t framebuffer_addr;
  uint32_t framebuffer_pitch;
  uint32_t framebuffer_width;
  uint32_t framebuffer_height;
  uint8_t framebuffer_bpp;
  uint8_t framebuffer_type;
  uint16_t reserved;
  /* Color info follows based on type */
};

/* Function prototypes */
void multiboot2_parse(uint64_t info_addr);
struct multiboot2_tag *multiboot2_find_tag(uint32_t type);
struct multiboot2_tag_mmap *multiboot2_get_mmap(void);
const char *multiboot2_get_cmdline(void);

/* Function prototypes */
void multiboot2_parse(uint64_t info_addr);
struct multiboot2_tag *multiboot2_find_tag(uint32_t type);
struct multiboot2_tag_mmap *multiboot2_get_mmap(void);
const char *multiboot2_get_cmdline(void);
uint64_t multiboot2_get_total_memory(void);

#endif /* MINIMALOS_MULTIBOOT2_H */
