#include "initrd.h"
#include "vfs.h"
#include "../lib/string.h"
#include "../lib/printk.h"
#include "../mm/heap.h"

#define MAX_INITRD_FILES 128

// TAR file entry
typedef struct {
    char name[256];
    u64 offset;
    u64 size;
    u32 type;
} initrd_file_t;

static initrd_file_t initrd_files[MAX_INITRD_FILES];
static u32 initrd_file_count = 0;
static uintptr initrd_base = 0;

// Operations for initrd files
static vfs_operations_t initrd_file_ops;
static vfs_operations_t initrd_dir_ops;

// Convert octal string to integer
static u64 tar_oct_to_int(const char* str, size_t size) {
    u64 result = 0;
    for (size_t i = 0; i < size && str[i] >= '0' && str[i] <= '7'; i++) {
        result = result * 8 + (str[i] - '0');
    }
    return result;
}

// Read from initrd file
static int initrd_read(vfs_node_t* node, u64 offset, u64 size, u8* buffer) {
    if (!node || !buffer) {
        return -1;
    }
    
    initrd_file_t* file = (initrd_file_t*)(uintptr)node->impl;
    if (!file) {
        return -1;
    }
    
    // Check bounds
    if (offset >= file->size) {
        return 0;
    }
    
    if (offset + size > file->size) {
        size = file->size - offset;
    }
    
    // Copy data
    u8* source = (u8*)(initrd_base + file->offset + offset);
    memcpy(buffer, source, size);
    
    return (int)size;
}

// Stub operations
static void initrd_open(vfs_node_t* node) {
    (void)node;
}

static void initrd_close(vfs_node_t* node) {
    (void)node;
}

// Read directory entry
static dirent_t* initrd_readdir(vfs_node_t* node, u32 index) {
    (void)node;
    
    if (index >= initrd_file_count) {
        return NULL;
    }
    
    static dirent_t dirent;
    strncpy(dirent.name, initrd_files[index].name, 127);
    dirent.name[127] = '\0';
    dirent.inode = index;
    
    return &dirent;
}

// Find file in directory
static vfs_node_t* initrd_finddir(vfs_node_t* node, const char* name) {
    (void)node;
    
    for (u32 i = 0; i < initrd_file_count; i++) {
        if (strcmp(initrd_files[i].name, name) == 0) {
            // Create VFS node for this file
            vfs_node_t* file_node = kmalloc(sizeof(vfs_node_t));
            if (!file_node) {
                return NULL;
            }
            
            memset(file_node, 0, sizeof(vfs_node_t));
            strncpy(file_node->name, initrd_files[i].name, 127);
            file_node->name[127] = '\0';
            file_node->inode = i;
            file_node->type = initrd_files[i].type;
            file_node->size = initrd_files[i].size;
            file_node->impl = (u64)(uintptr)&initrd_files[i];
            file_node->ops = &initrd_file_ops;
            
            return file_node;
        }
    }
    
    return NULL;
}

vfs_node_t* initrd_init(uintptr tar_address, size_t tar_size) {
    printk("[INITRD] Initializing from TAR archive at 0x%lx (size: %lu bytes)\n", 
           tar_address, tar_size);
    
    initrd_base = tar_address;
    initrd_file_count = 0;
    
    // Setup operations
    initrd_file_ops.read = initrd_read;
    initrd_file_ops.write = NULL;
    initrd_file_ops.open = initrd_open;
    initrd_file_ops.close = initrd_close;
    initrd_file_ops.readdir = NULL;
    initrd_file_ops.finddir = NULL;
    
    initrd_dir_ops.read = NULL;
    initrd_dir_ops.write = NULL;
    initrd_dir_ops.open = initrd_open;
    initrd_dir_ops.close = initrd_close;
    initrd_dir_ops.readdir = initrd_readdir;
    initrd_dir_ops.finddir = initrd_finddir;
    
    // Parse TAR archive
    uintptr current = tar_address;
    uintptr end = tar_address + tar_size;
    
    while (current < end && initrd_file_count < MAX_INITRD_FILES) {
        struct tar_header* header = (struct tar_header*)current;
        
        // Check for end of archive (empty block)
        if (header->filename[0] == '\0') {
            break;
        }
        
        // Check magic
        if (strncmp(header->magic, "ustar", 5) != 0) {
            printk("[INITRD] Invalid TAR magic at offset 0x%lx\n", current - tar_address);
            break;
        }
        
        // Get file size
        u64 file_size = tar_oct_to_int(header->size, 12);
        
        // Store file information
        strncpy(initrd_files[initrd_file_count].name, header->filename, 255);
        initrd_files[initrd_file_count].name[255] = '\0';
        initrd_files[initrd_file_count].offset = current + 512 - tar_address;
        initrd_files[initrd_file_count].size = file_size;
        
        // Determine type
        if (header->typeflag == '5' || header->typeflag == '0') {
            initrd_files[initrd_file_count].type = (header->typeflag == '5') ? VFS_DIRECTORY : VFS_FILE;
        } else {
            initrd_files[initrd_file_count].type = VFS_FILE;
        }
        
        printk("[INITRD]   File: %s (size: %lu bytes, type: %c)\n", 
               initrd_files[initrd_file_count].name, file_size, header->typeflag);
        
        initrd_file_count++;
        
        // Move to next header (aligned to 512 bytes)
        current += 512;
        if (file_size > 0) {
            current += ((file_size + 511) / 512) * 512;
        }
    }
    
    printk("[INITRD] Loaded %u files from ramdisk\n", initrd_file_count);
    
    // Create root VFS node
    vfs_node_t* root = kmalloc(sizeof(vfs_node_t));
    if (!root) {
        printk("[INITRD] Failed to allocate root node\n");
        return NULL;
    }
    
    memset(root, 0, sizeof(vfs_node_t));
    strcpy(root->name, "initrd");
    root->type = VFS_DIRECTORY;
    root->ops = &initrd_dir_ops;
    
    return root;
}
