#ifndef _MULTIBOOT2_H
#define _MULTIBOOT2_H

#include <stdint.h>

/* Multiboot2 magic value passed by bootloader */
#define MULTIBOOT2_MAGIC 0x36D76289

/* Tag types */
#define MULTIBOOT2_TAG_END              0
#define MULTIBOOT2_TAG_CMDLINE          1
#define MULTIBOOT2_TAG_BOOTLOADER       2
#define MULTIBOOT2_TAG_MODULE           3
#define MULTIBOOT2_TAG_BASIC_MEMINFO    4
#define MULTIBOOT2_TAG_BOOTDEV          5
#define MULTIBOOT2_TAG_MMAP             6
#define MULTIBOOT2_TAG_VBE              7
#define MULTIBOOT2_TAG_FRAMEBUFFER      8
#define MULTIBOOT2_TAG_ELF_SECTIONS     9
#define MULTIBOOT2_TAG_APM              10
#define MULTIBOOT2_TAG_EFI32            11
#define MULTIBOOT2_TAG_EFI64            12
#define MULTIBOOT2_TAG_SMBIOS           13
#define MULTIBOOT2_TAG_ACPI_OLD         14
#define MULTIBOOT2_TAG_ACPI_NEW         15
#define MULTIBOOT2_TAG_NETWORK          16
#define MULTIBOOT2_TAG_EFI_MMAP         17
#define MULTIBOOT2_TAG_EFI_BS           18
#define MULTIBOOT2_TAG_EFI32_IH         19
#define MULTIBOOT2_TAG_EFI64_IH         20
#define MULTIBOOT2_TAG_LOAD_BASE_ADDR   21

/* Memory map entry types */
#define MULTIBOOT2_MMAP_AVAILABLE        1
#define MULTIBOOT2_MMAP_RESERVED         2
#define MULTIBOOT2_MMAP_ACPI_RECLAIMABLE 3
#define MULTIBOOT2_MMAP_NVS              4
#define MULTIBOOT2_MMAP_BADRAM           5

/* Fixed boot info header */
typedef struct {
    uint32_t total_size;
    uint32_t reserved;
} __attribute__((packed)) multiboot2_info_t;

/* Tag header */
typedef struct {
    uint32_t type;
    uint32_t size;
} __attribute__((packed)) multiboot2_tag_t;

/* Basic memory info tag */
typedef struct {
    uint32_t type;
    uint32_t size;
    uint32_t mem_lower;  /* KB */
    uint32_t mem_upper;  /* KB */
} __attribute__((packed)) multiboot2_tag_basic_meminfo_t;

/* Memory map entry */
typedef struct {
    uint64_t base_addr;
    uint64_t length;
    uint32_t type;
    uint32_t reserved;
} __attribute__((packed)) multiboot2_mmap_entry_t;

/* Memory map tag */
typedef struct {
    uint32_t type;
    uint32_t size;
    uint32_t entry_size;
    uint32_t entry_version;
    /* entries follow */
} __attribute__((packed)) multiboot2_tag_mmap_t;

/* Framebuffer tag */
typedef struct {
    uint32_t type;
    uint32_t size;
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
    uint16_t reserved;
} __attribute__((packed)) multiboot2_tag_framebuffer_t;

/* Find a tag by type */
multiboot2_tag_t *multiboot2_find_tag(uint64_t mb_info_addr, uint32_t tag_type);

/* Parse and print memory map */
void multiboot2_print_mmap(uint64_t mb_info_addr);

/* Get total usable memory in bytes */
uint64_t multiboot2_get_memory_size(uint64_t mb_info_addr);

#endif /* _MULTIBOOT2_H */
