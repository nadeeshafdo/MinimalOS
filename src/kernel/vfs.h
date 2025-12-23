#ifndef VFS_H
#define VFS_H

#include "stdint.h"
#include "stddef.h"

#define MAX_FILES 32
#define MAX_FILENAME 32
#define MAX_FILESIZE 4096

typedef int bool;
#define true 1
#define false 0

typedef struct {
    char name[MAX_FILENAME];
    char* data;
    size_t size;
    uint32_t created;  // Timestamp in seconds since boot
    bool is_used;
} file_t;

// Initialize VFS
void vfs_init(void);

// Create a new file
int vfs_create(const char* name);

// Write data to a file
int vfs_write(const char* name, const char* data, size_t size);

// Read data from a file
int vfs_read(const char* name, char* buffer, size_t max_size);

// Delete a file
int vfs_delete(const char* name);

// List all files
file_t* vfs_list(int* count);

// Get file by name
file_t* vfs_get(const char* name);

#endif
