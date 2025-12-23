#include "vfs.h"
#include "memory.h"
#include "timer.h"

static file_t files[MAX_FILES];
static int file_count = 0;

// String functions
static int vfs_strcmp(const char* s1, const char* s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *s1 - *s2;
}

static void vfs_strcpy(char* dest, const char* src) {
    while (*src) {
        *dest++ = *src++;
    }
    *dest = '\0';
}

static void vfs_memcpy(void* dest, const void* src, size_t n) {
    char* d = (char*)dest;
    const char* s = (const char*)src;
    while (n--) *d++ = *s++;
}

void vfs_init(void) {
    for (int i = 0; i < MAX_FILES; i++) {
        files[i].is_used = false;
        files[i].data = 0;
        files[i].size = 0;
    }
    file_count = 0;
}

int vfs_create(const char* name) {
    // Check if file already exists
    for (int i = 0; i < MAX_FILES; i++) {
        if (files[i].is_used && vfs_strcmp(files[i].name, name) == 0) {
            return -1; // File already exists
        }
    }
    
    // Find free slot
    for (int i = 0; i < MAX_FILES; i++) {
        if (!files[i].is_used) {
            vfs_strcpy(files[i].name, name);
            files[i].data = 0;
            files[i].size = 0;
            files[i].created = get_uptime_seconds();
            files[i].is_used = true;
            file_count++;
            return 0; // Success
        }
    }
    
    return -2; // No free slots
}

int vfs_write(const char* name, const char* data, size_t size) {
    if (size > MAX_FILESIZE) return -1;
    
    // Find file
    file_t* file = vfs_get(name);
    if (!file) return -2; // File not found
    
    // Free old data if exists
    if (file->data) {
        free(file->data);
    }
    
    // Allocate new data
    file->data = (char*)malloc(size + 1);
    if (!file->data) return -3; // Out of memory
    
    vfs_memcpy(file->data, data, size);
    file->data[size] = '\0';
    file->size = size;
    
    return 0; // Success
}

int vfs_read(const char* name, char* buffer, size_t max_size) {
    file_t* file = vfs_get(name);
    if (!file) return -1; // File not found
    if (!file->data) return 0; // Empty file
    
    size_t copy_size = file->size < max_size ? file->size : max_size;
    vfs_memcpy(buffer, file->data, copy_size);
    
    if (copy_size < max_size) {
        buffer[copy_size] = '\0';
    }
    
    return copy_size;
}

int vfs_delete(const char* name) {
    for (int i = 0; i < MAX_FILES; i++) {
        if (files[i].is_used && vfs_strcmp(files[i].name, name) == 0) {
            if (files[i].data) {
                free(files[i].data);
            }
            files[i].is_used = false;
            files[i].data = 0;
            files[i].size = 0;
            file_count--;
            return 0; // Success
        }
    }
    return -1; // File not found
}

file_t* vfs_list(int* count) {
    *count = file_count;
    return files;
}

file_t* vfs_get(const char* name) {
    for (int i = 0; i < MAX_FILES; i++) {
        if (files[i].is_used && vfs_strcmp(files[i].name, name) == 0) {
            return &files[i];
        }
    }
    return 0;
}
