/* Initial RAM Disk (initrd) Filesystem */

#include <stdint.h>
#include <stddef.h>
#include "initrd.h"
#include "vfs.h"
#include "kheap.h"

/* Initrd file nodes */
static vfs_node_t *initrd_root = (void*)0;
static vfs_node_t *initrd_files[INITRD_MAX_FILES];
static uint32_t initrd_num_files = 0;

/* File data pointers */
static uint8_t *file_data[INITRD_MAX_FILES];
static uint32_t file_sizes[INITRD_MAX_FILES];

/* String comparison */
static int initrd_strcmp(const char *s1, const char *s2) {
    while (*s1 && *s2 && *s1 == *s2) { s1++; s2++; }
    return *s1 == *s2;
}

/* Read file */
static size_t initrd_read(vfs_node_t *node, size_t offset, size_t size, uint8_t *buffer) {
    uint32_t idx = (uint32_t)(uint64_t)node->private_data;
    if (idx >= initrd_num_files) return 0;
    
    uint32_t file_size = file_sizes[idx];
    if (offset >= file_size) return 0;
    
    if (offset + size > file_size) {
        size = file_size - offset;
    }
    
    uint8_t *src = file_data[idx] + offset;
    for (size_t i = 0; i < size; i++) {
        buffer[i] = src[i];
    }
    
    return size;
}

/* Read directory entry */
static vfs_node_t *initrd_readdir(vfs_node_t *node, uint32_t index) {
    (void)node;
    if (index >= initrd_num_files) return (void*)0;
    return initrd_files[index];
}

/* Find file in directory */
static vfs_node_t *initrd_finddir(vfs_node_t *node, const char *name) {
    (void)node;
    for (uint32_t i = 0; i < initrd_num_files; i++) {
        if (initrd_strcmp(initrd_files[i]->name, name)) {
            return initrd_files[i];
        }
    }
    return (void*)0;
}

/* Operations for root directory */
static vfs_ops_t root_ops = {
    .read = (void*)0,
    .write = (void*)0,
    .readdir = initrd_readdir,
    .finddir = initrd_finddir
};

/* Operations for files */
static vfs_ops_t file_ops = {
    .read = initrd_read,
    .write = (void*)0,
    .readdir = (void*)0,
    .finddir = (void*)0
};

/* String copy helper */
static void initrd_strcpy(char *dst, const char *src, size_t max) {
    size_t i = 0;
    while (src[i] && i < max - 1) {
        dst[i] = src[i];
        i++;
    }
    dst[i] = '\0';
}

vfs_node_t *initrd_init(uint64_t location) {
    initrd_header_t *header = (initrd_header_t *)location;
    
    /* Verify magic */
    if (header->magic != INITRD_MAGIC) {
        return (void*)0;
    }
    
    initrd_num_files = header->num_files;
    if (initrd_num_files > INITRD_MAX_FILES) {
        initrd_num_files = INITRD_MAX_FILES;
    }
    
    /* Create root node */
    initrd_root = (vfs_node_t *)kmalloc(sizeof(vfs_node_t));
    if (!initrd_root) return (void*)0;
    
    initrd_strcpy(initrd_root->name, "/", VFS_MAX_NAME);
    initrd_root->type = VFS_DIRECTORY;
    initrd_root->flags = VFS_READ;
    initrd_root->size = 0;
    initrd_root->inode = 0;
    initrd_root->private_data = (void*)0;
    initrd_root->ops = &root_ops;
    initrd_root->parent = (void*)0;
    
    /* Parse file headers */
    uint64_t offset = sizeof(initrd_header_t);
    
    for (uint32_t i = 0; i < initrd_num_files; i++) {
        initrd_file_header_t *file_header = (initrd_file_header_t *)(location + offset);
        offset += sizeof(initrd_file_header_t);
        
        /* Store file data pointer */
        file_data[i] = (uint8_t *)(location + offset);
        file_sizes[i] = file_header->size;
        offset += file_header->size;
        
        /* Create file node */
        initrd_files[i] = (vfs_node_t *)kmalloc(sizeof(vfs_node_t));
        if (!initrd_files[i]) continue;
        
        initrd_strcpy(initrd_files[i]->name, file_header->name, VFS_MAX_NAME);
        initrd_files[i]->type = VFS_FILE;
        initrd_files[i]->flags = VFS_READ;
        initrd_files[i]->size = file_header->size;
        initrd_files[i]->inode = i + 1;
        initrd_files[i]->private_data = (void*)(uint64_t)i;
        initrd_files[i]->ops = &file_ops;
        initrd_files[i]->parent = initrd_root;
    }
    
    return initrd_root;
}

/* Get number of files (for ls command) */
uint32_t initrd_get_file_count(void) {
    return initrd_num_files;
}
