#ifndef _VFS_H
#define _VFS_H

#include <stdint.h>
#include <stddef.h>

/* File types */
#define VFS_FILE        1
#define VFS_DIRECTORY   2

/* File open flags */
#define VFS_READ        1
#define VFS_WRITE       2

/* Maximum path length */
#define VFS_MAX_PATH    256
#define VFS_MAX_NAME    64

/* Forward declarations */
struct vfs_node;

/* File system operations */
typedef struct {
    size_t (*read)(struct vfs_node *node, size_t offset, size_t size, uint8_t *buffer);
    size_t (*write)(struct vfs_node *node, size_t offset, size_t size, uint8_t *buffer);
    struct vfs_node *(*readdir)(struct vfs_node *node, uint32_t index);
    struct vfs_node *(*finddir)(struct vfs_node *node, const char *name);
} vfs_ops_t;

/* VFS node (file or directory) */
typedef struct vfs_node {
    char name[VFS_MAX_NAME];
    uint32_t type;
    uint32_t flags;
    uint64_t size;
    uint64_t inode;
    void *private_data;
    vfs_ops_t *ops;
    struct vfs_node *parent;
} vfs_node_t;

/* Initialize VFS */
void vfs_init(void);

/* Mount a filesystem at root */
void vfs_mount_root(vfs_node_t *root);

/* Get root node */
vfs_node_t *vfs_get_root(void);

/* File operations */
size_t vfs_read(vfs_node_t *node, size_t offset, size_t size, uint8_t *buffer);
size_t vfs_write(vfs_node_t *node, size_t offset, size_t size, uint8_t *buffer);

/* Directory operations */
vfs_node_t *vfs_readdir(vfs_node_t *node, uint32_t index);
vfs_node_t *vfs_finddir(vfs_node_t *node, const char *name);

/* Path resolution */
vfs_node_t *vfs_lookup(const char *path);

#endif /* _VFS_H */
