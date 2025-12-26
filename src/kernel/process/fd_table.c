#include "fd_table.h"
#include "../lib/string.h"

void fd_table_init(fd_table_t* table) {
    if (!table) {
        return;
    }
    
    memset(table, 0, sizeof(fd_table_t));
    
    // Mark standard streams as in use (even though they're not connected yet)
    table->fds[STDIN].in_use = true;
    table->fds[STDOUT].in_use = true;
    table->fds[STDERR].in_use = true;
}

int fd_alloc(fd_table_t* table, vfs_node_t* node, u32 flags) {
    if (!table || !node) {
        return -1;
    }
    
    // Find free file descriptor (skip stdin/stdout/stderr for now)
    for (int i = 3; i < MAX_FDS; i++) {
        if (!table->fds[i].in_use) {
            table->fds[i].node = node;
            table->fds[i].position = 0;
            table->fds[i].flags = flags;
            table->fds[i].in_use = true;
            return i;
        }
    }
    
    return -1;  // No free descriptors
}

void fd_free(fd_table_t* table, int fd) {
    if (!table || fd < 0 || fd >= MAX_FDS) {
        return;
    }
    
    if (table->fds[fd].in_use) {
        // Close the VFS node
        if (table->fds[fd].node) {
            vfs_close(table->fds[fd].node);
        }
        
        // Mark as free
        table->fds[fd].node = NULL;
        table->fds[fd].position = 0;
        table->fds[fd].flags = 0;
        table->fds[fd].in_use = false;
    }
}

file_descriptor_t* fd_get(fd_table_t* table, int fd) {
    if (!table || fd < 0 || fd >= MAX_FDS) {
        return NULL;
    }
    
    if (!table->fds[fd].in_use) {
        return NULL;
    }
    
    return &table->fds[fd];
}
