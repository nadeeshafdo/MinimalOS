#include "pmm.h"
#include "../lib/string.h"
#include "../lib/printk.h"

// Bitmap for tracking free/used frames
static u8* frame_bitmap = NULL;
static size_t total_frames = 0;
static size_t used_frames = 0;
static u64 total_memory = 0;

// Kernel end address (defined in linker script)
extern u8 _kernel_end[];

// Bitmap base (placed after kernel)
#define BITMAP_BASE ((uintptr)_kernel_end)

// Multiboot2 tag structures
struct multiboot_tag {
    u32 type;
    u32 size;
};

struct multiboot_tag_mmap {
    u32 type;
    u32 size;
    u32 entry_size;
    u32 entry_version;
    struct multiboot_mmap_entry entries[];
};

static inline void bitmap_set_frame(size_t frame) {
    size_t byte = frame / 8;
    size_t bit = frame % 8;
    frame_bitmap[byte] |= (1 << bit);
}

static inline void bitmap_clear_frame(size_t frame) {
    size_t byte = frame / 8;
    size_t bit = frame % 8;
    frame_bitmap[byte] &= ~(1 << bit);
}

static inline bool bitmap_test_frame(size_t frame) {
    size_t byte = frame / 8;
    size_t bit = frame % 8;
    return (frame_bitmap[byte] & (1 << bit)) != 0;
}

static size_t find_free_frame(void) {
    for (size_t i = 0; i < total_frames; i++) {
        if (!bitmap_test_frame(i)) {
            return i;
        }
    }
    return (size_t)-1;
}

static size_t find_free_frames(size_t count) {
    size_t consecutive = 0;
    size_t start = 0;
    
    for (size_t i = 0; i < total_frames; i++) {
        if (!bitmap_test_frame(i)) {
            if (consecutive == 0) {
                start = i;
            }
            consecutive++;
            if (consecutive == count) {
                return start;
            }
        } else {
            consecutive = 0;
        }
    }
    return (size_t)-1;
}

void pmm_init(void* mbi_ptr) {
    struct multiboot_tag* tag;
    struct multiboot_tag_mmap* mmap_tag = NULL;
    u32 total_size = *(u32*)mbi_ptr;
    
    printk("[PMM] Initializing physical memory manager...\n");
    printk("[PMM] Multiboot info at: %p (size: %u bytes)\n", mbi_ptr, total_size);
    
    // Find memory map tag
    tag = (struct multiboot_tag*)((u8*)mbi_ptr + 8);
    while (tag->type != 0 && (u8*)tag < (u8*)mbi_ptr + total_size) {
        if (tag->type == 6) {  // Memory map tag
            mmap_tag = (struct multiboot_tag_mmap*)tag;
            break;
        }
        tag = (struct multiboot_tag*)((u8*)tag + ((tag->size + 7) & ~7));
    }
    
    if (mmap_tag == NULL) {
        printk("[PMM] WARNING: No memory map found, using default 128MB\n");
        total_memory = 128 * 1024 * 1024;
    }
    
    // Calculate total memory and maximum address
    u64 max_addr = 0;
    
    if (mmap_tag != NULL) {
        total_memory = 0; // Reset, will be calculated from memory map
        size_t entry_count = (mmap_tag->size - sizeof(struct multiboot_tag_mmap)) / mmap_tag->entry_size;
        
        printk("[PMM] Memory map entries: %u\n", (u32)entry_count);
        
        for (size_t i = 0; i < entry_count; i++) {
            struct multiboot_mmap_entry* entry = &mmap_tag->entries[i];
            
            if (entry->type == MULTIBOOT_MEMORY_AVAILABLE) {
                total_memory += entry->len;
                u64 end_addr = entry->addr + entry->len;
                if (end_addr > max_addr) {
                    max_addr = end_addr;
                }
                
                printk("[PMM]   [%lx - %lx] Available (%lu KB)\n", 
                       entry->addr, end_addr - 1, entry->len / 1024);
            } else {
                printk("[PMM]   [%lx - %lx] Reserved (type %u)\n", 
                       entry->addr, entry->addr + entry->len - 1, entry->type);
            }
        }
    }
    
    // Use total_memory as max if we didn't find specific regions
    if (max_addr == 0) {
        max_addr = total_memory;
    }
    
    // Calculate number of frames
    total_frames = max_addr / PAGE_SIZE;
    size_t bitmap_size = (total_frames + 7) / 8;
    
    printk("[PMM] Total memory: %lu MB\n", total_memory / (1024 * 1024));
    printk("[PMM] Maximum address: %lx\n", max_addr);
    printk("[PMM] Total frames: %u\n", (u32)total_frames);
    printk("[PMM] Bitmap size: %u bytes\n", (u32)bitmap_size);
    
    // Place bitmap after kernel
    frame_bitmap = (u8*)BITMAP_BASE;
    printk("[PMM] Bitmap at: %p\n", frame_bitmap);
    
    // Mark all frames as used initially
    memset(frame_bitmap, 0xFF, bitmap_size);
    used_frames = total_frames;
    
    // Mark available regions as free (if we have memmap)
    if (mmap_tag != NULL) {
        size_t entry_count = (mmap_tag->size - sizeof(struct multiboot_tag_mmap)) / mmap_tag->entry_size;
        for (size_t i = 0; i < entry_count; i++) {
            struct multiboot_mmap_entry* entry = &mmap_tag->entries[i];
            
            if (entry->type == MULTIBOOT_MEMORY_AVAILABLE) {
                u64 start_frame = entry->addr / PAGE_SIZE;
                u64 end_frame = (entry->addr + entry->len) / PAGE_SIZE;
                
                for (u64 frame = start_frame; frame < end_frame; frame++) {
                    bitmap_clear_frame(frame);
                    used_frames--;
                }
            }
        }
    } else {
        // No memory map, mark all as free except kernel
        memset(frame_bitmap, 0x00, bitmap_size);
        used_frames = 0;
    }
    
    // Reserve kernel area (0 to kernel_end + bitmap)
    u64 kernel_end_addr = (u64)frame_bitmap + bitmap_size;
    u64 kernel_frames = (kernel_end_addr + PAGE_SIZE - 1) / PAGE_SIZE;
    
    printk("[PMM] Kernel area: 0x0 - %lx (%lu frames)\n", 
           kernel_end_addr, kernel_frames);
    
    for (u64 frame = 0; frame < kernel_frames; frame++) {
        if (!bitmap_test_frame(frame)) {
            bitmap_set_frame(frame);
            used_frames++;
        }
    }
    
    printk("[PMM] Free memory: %lu MB\n", pmm_get_free_memory() / (1024 * 1024));
    printk("[PMM] Used memory: %lu MB\n", pmm_get_used_memory() / (1024 * 1024));
    printk("[PMM] Initialization complete!\n");
}

uintptr pmm_alloc_frame(void) {
    size_t frame = find_free_frame();
    if (frame == (size_t)-1) {
        return 0;  // Out of memory
    }
    
    bitmap_set_frame(frame);
    used_frames++;
    
    return frame * PAGE_SIZE;
}

void pmm_free_frame(uintptr frame_addr) {
    size_t frame = frame_addr / PAGE_SIZE;
    
    if (frame >= total_frames) {
        return;  // Invalid frame
    }
    
    if (!bitmap_test_frame(frame)) {
        return;  // Already free
    }
    
    bitmap_clear_frame(frame);
    used_frames--;
}

uintptr pmm_alloc_frames(size_t count) {
    if (count == 0) {
        return 0;
    }
    
    if (count == 1) {
        return pmm_alloc_frame();
    }
    
    size_t start_frame = find_free_frames(count);
    if (start_frame == (size_t)-1) {
        return 0;  // Not enough contiguous frames
    }
    
    // Mark all frames as used
    for (size_t i = 0; i < count; i++) {
        bitmap_set_frame(start_frame + i);
        used_frames++;
    }
    
    return start_frame * PAGE_SIZE;
}

void pmm_free_frames(uintptr frame_addr, size_t count) {
    for (size_t i = 0; i < count; i++) {
        pmm_free_frame(frame_addr + (i * PAGE_SIZE));
    }
}

u64 pmm_get_total_memory(void) {
    return total_memory;
}

u64 pmm_get_free_memory(void) {
    return (total_frames - used_frames) * PAGE_SIZE;
}

u64 pmm_get_used_memory(void) {
    return used_frames * PAGE_SIZE;
}
