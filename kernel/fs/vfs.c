/* Virtual File System */

#include <stdint.h>
#include <stddef.h>
#include "vfs.h"

/* Root node */
static vfs_node_t *vfs_root = (void*)0;

void vfs_init(void) {
    vfs_root = (void*)0;
}

void vfs_mount_root(vfs_node_t *root) {
    vfs_root = root;
}

vfs_node_t *vfs_get_root(void) {
    return vfs_root;
}

size_t vfs_read(vfs_node_t *node, size_t offset, size_t size, uint8_t *buffer) {
    if (!node || !node->ops || !node->ops->read) return 0;
    return node->ops->read(node, offset, size, buffer);
}

size_t vfs_write(vfs_node_t *node, size_t offset, size_t size, uint8_t *buffer) {
    if (!node || !node->ops || !node->ops->write) return 0;
    return node->ops->write(node, offset, size, buffer);
}

vfs_node_t *vfs_readdir(vfs_node_t *node, uint32_t index) {
    if (!node || !node->ops || !node->ops->readdir) return (void*)0;
    if (node->type != VFS_DIRECTORY) return (void*)0;
    return node->ops->readdir(node, index);
}

vfs_node_t *vfs_finddir(vfs_node_t *node, const char *name) {
    if (!node || !node->ops || !node->ops->finddir) return (void*)0;
    if (node->type != VFS_DIRECTORY) return (void*)0;
    return node->ops->finddir(node, name);
}

/* String comparison helper */
static int str_equals(const char *s1, const char *s2) {
    while (*s1 && *s2) {
        if (*s1 != *s2) return 0;
        s1++; s2++;
    }
    return *s1 == *s2;
}

vfs_node_t *vfs_lookup(const char *path) {
    if (!path || !vfs_root) return (void*)0;
    
    /* Skip leading slash */
    if (*path == '/') path++;
    if (*path == '\0') return vfs_root;
    
    vfs_node_t *current = vfs_root;
    char component[VFS_MAX_NAME];
    int i = 0;
    
    while (*path) {
        /* Extract path component */
        i = 0;
        while (*path && *path != '/' && i < VFS_MAX_NAME - 1) {
            component[i++] = *path++;
        }
        component[i] = '\0';
        
        if (*path == '/') path++;
        
        if (i == 0) continue;
        
        /* Handle . and .. */
        if (str_equals(component, ".")) continue;
        if (str_equals(component, "..")) {
            if (current->parent) current = current->parent;
            continue;
        }
        
        /* Find in current directory */
        current = vfs_finddir(current, component);
        if (!current) return (void*)0;
    }
    
    return current;
}
