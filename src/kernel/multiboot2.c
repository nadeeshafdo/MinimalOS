/**
 * MinimalOS - Multiboot2 Information Parser
 */

#include <minimalos/multiboot2.h>
#include <minimalos/types.h>

extern void printk(const char *fmt, ...);

/* Stored pointers to key tags */
static struct multiboot2_info *mb2_info = NULL;
static struct multiboot2_tag_mmap *mmap_tag = NULL;
static const char *cmdline = NULL;

/**
 * Find a specific tag in the Multiboot2 info structure
 */
struct multiboot2_tag *multiboot2_find_tag(uint32_t type) {
  if (!mb2_info)
    return NULL;

  struct multiboot2_tag *tag =
      (struct multiboot2_tag *)((uint8_t *)mb2_info + 8);

  while (tag->type != MULTIBOOT2_TAG_END) {
    if (tag->type == type) {
      return tag;
    }
    /* Advance to next tag (8-byte aligned) */
    uintptr_t next = (uintptr_t)tag + tag->size;
    next = (next + 7) & ~7;
    tag = (struct multiboot2_tag *)next;
  }

  return NULL;
}

/**
 * Parse Multiboot2 information structure
 * @param info_addr Physical address of the info structure
 */
void multiboot2_parse(uint64_t info_addr) {
  /* Convert physical address to virtual address (higher half) */
  mb2_info = (struct multiboot2_info *)PHYS_TO_VIRT(info_addr);

  printk("  Multiboot2 info size: %u bytes\n", mb2_info->total_size);

  /* Find and cache important tags */
  struct multiboot2_tag *tag =
      (struct multiboot2_tag *)((uint8_t *)mb2_info + 8);

  while (tag->type != MULTIBOOT2_TAG_END) {
    switch (tag->type) {
    case MULTIBOOT2_TAG_CMDLINE: {
      struct multiboot2_tag_string *str_tag =
          (struct multiboot2_tag_string *)tag;
      cmdline = str_tag->string;
      printk("  Command line: %s\n", cmdline[0] ? cmdline : "(empty)");
      break;
    }

    case MULTIBOOT2_TAG_BOOTLOADER_NAME: {
      struct multiboot2_tag_string *str_tag =
          (struct multiboot2_tag_string *)tag;
      printk("  Bootloader: %s\n", str_tag->string);
      break;
    }

    case MULTIBOOT2_TAG_BASIC_MEMINFO: {
      struct multiboot2_tag_basic_meminfo *mem =
          (struct multiboot2_tag_basic_meminfo *)tag;
      printk("  Memory: lower=%u KB, upper=%u KB\n", mem->mem_lower,
             mem->mem_upper);
      break;
    }

    case MULTIBOOT2_TAG_MMAP: {
      mmap_tag = (struct multiboot2_tag_mmap *)tag;
      printk("  Memory map entries: %u\n",
             (mmap_tag->size - 16) / mmap_tag->entry_size);

      /* Print memory map */
      struct multiboot2_mmap_entry *entry = mmap_tag->entries;
      uintptr_t entries_end = (uintptr_t)mmap_tag + mmap_tag->size;

      while ((uintptr_t)entry < entries_end) {
        const char *type_str;
        switch (entry->type) {
        case MULTIBOOT2_MEMORY_AVAILABLE:
          type_str = "Available";
          break;
        case MULTIBOOT2_MEMORY_RESERVED:
          type_str = "Reserved";
          break;
        case MULTIBOOT2_MEMORY_ACPI_RECLAIMABLE:
          type_str = "ACPI Reclaimable";
          break;
        case MULTIBOOT2_MEMORY_NVS:
          type_str = "ACPI NVS";
          break;
        case MULTIBOOT2_MEMORY_BADRAM:
          type_str = "Bad RAM";
          break;
        default:
          type_str = "Unknown";
          break;
        }

        printk("    0x%lx - 0x%lx (%s)\n", entry->addr,
               entry->addr + entry->len, type_str);

        entry = (struct multiboot2_mmap_entry *)((uintptr_t)entry +
                                                 mmap_tag->entry_size);
      }
      break;
    }

    case MULTIBOOT2_TAG_ACPI_OLD:
      printk("  ACPI RSDP v1.0 found\n");
      break;

    case MULTIBOOT2_TAG_ACPI_NEW:
      printk("  ACPI RSDP v2.0+ found\n");
      break;
    }

    /* Advance to next tag (8-byte aligned) */
    uintptr_t next = (uintptr_t)tag + tag->size;
    next = (next + 7) & ~7;
    tag = (struct multiboot2_tag *)next;
  }
}

/**
 * Get the memory map tag
 */
struct multiboot2_tag_mmap *multiboot2_get_mmap(void) { return mmap_tag; }

/**
 * Get the command line string
 */
const char *multiboot2_get_cmdline(void) { return cmdline; }

/**
 * Get total available memory (in bytes)
 */
uint64_t multiboot2_get_total_memory(void) {
  if (!mmap_tag)
    return 0;

  uint64_t total = 0;
  struct multiboot2_mmap_entry *entry = mmap_tag->entries;
  uintptr_t entries_end = (uintptr_t)mmap_tag + mmap_tag->size;

  while ((uintptr_t)entry < entries_end) {
    if (entry->type == MULTIBOOT2_MEMORY_AVAILABLE) {
      total += entry->len;
    }
    entry = (struct multiboot2_mmap_entry *)((uintptr_t)entry +
                                             mmap_tag->entry_size);
  }

  return total;
}
