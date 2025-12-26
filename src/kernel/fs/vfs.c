#include "vfs.h"
#include "../lib/string.h"
#include "../lib/printk.h"
#include "../mm/heap.h"

static vfs_node_t* vfs_root = NULL;

void vfs_init(void) {
    printk("[VFS] Initializing Virtual Filesystem...\n");
    vfs_root = NULL;
    printk("[VFS] Initialization complete\n");
}

void vfs_mount(const char* path, vfs_node_t* node) {
    if (strcmp(path, "/") == 0) {
        vfs_root = node;
        printk("[VFS] Mounted root filesystem\n");
    } else {
        printk("[VFS] Mount path '%s' not yet supported\n", path);
    }
}

vfs_node_t* vfs_get_root(void) {
    return vfs_root;
}

static vfs_node_t* vfs_resolve_path(const char* path) {
    if (!vfs_root) {
        return NULL;
    }
    
    // Handle root directory
    if (strcmp(path, "/") == 0) {
        return vfs_root;
    }
    
    // Skip leading slash
    if (path[0] == '/') {
        path++;
    }
    
    // Start from root
    vfs_node_t* current = vfs_root;
    char component[128];
    u32 i = 0;
    
    while (*path) {
        // Extract path component
        i = 0;
        while (*path && *path != '/' && i < 127) {
            component[i++] = *path++;
        }
        component[i] = '\0';
        
        // Skip trailing slashes
        while (*path == '/') {
            path++;
        }
        
        // Find component in current directory
        if (current->ops && current->ops->finddir) {
            current = current->ops->finddir(current, component);
            if (!current) {
                return NULL;
            }
        } else {
            return NULL;
        }
    }
    
    return current;
}

vfs_node_t* vfs_open(const char* path) {
    vfs_node_t* node = vfs_resolve_path(path);
    if (node && node->ops && node->ops->open) {
        node->ops->open(node);
    }
    return node;
}

void vfs_close(vfs_node_t* node) {
    if (node && node->ops && node->ops->close) {
        node->ops->close(node);
    }
}

int vfs_read(vfs_node_t* node, u64 offset, u64 size, u8* buffer) {
    if (!node || !buffer) {
        return -1;
    }
    
    if (node->ops && node->ops->read) {
        return node->ops->read(node, offset, size, buffer);
    }
    
    return -1;
}

int vfs_write(vfs_node_t* node, u64 offset, u64 size, const u8* buffer) {
    if (!node || !buffer) {
        return -1;
    }
    
    if (node->ops && node->ops->write) {
        return node->ops->write(node, offset, size, buffer);
    }
    
    return -1;
}

dirent_t* vfs_readdir(vfs_node_t* node, u32 index) {
    if (!node || !(node->type & VFS_DIRECTORY)) {
        return NULL;
    }
    
    if (node->ops && node->ops->readdir) {
        return node->ops->readdir(node, index);
    }
    
    return NULL;
}

vfs_node_t* vfs_finddir(vfs_node_t* node, const char* name) {
    if (!node || !(node->type & VFS_DIRECTORY)) {
        return NULL;
    }
    
    if (node->ops && node->ops->finddir) {
        return node->ops->finddir(node, name);
    }
    
    return NULL;
}
