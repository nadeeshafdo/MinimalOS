#include "paging.h"

void setup_paging() {
    // Bootloader already set up basic identity paging
    // We can extend this later if needed for user space
}

void map_user_stack(uint64_t addr, size_t size) {
    // For now, assume bootloader identity mapping covers this
    // In a full implementation, we would map physical pages to virtual addresses
    // using the existing page tables
}