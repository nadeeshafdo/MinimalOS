#ifndef VFS_H
#define VFS_H

#include "../include/types.h"

// VFS node types
#define VFS_FILE        0x01
#define VFS_DIRECTORY   0x02
#define VFS_CHARDEVICE  0x03
#define VFS_BLOCKDEVICE 0x04
#define VFS_PIPE        0x05
#define VFS_SYMLINK     0x06
#define VFS_MOUNTPOINT  0x08

// Forward declarations
typedef struct vfs_node vfs_node_t;
typedef struct vfs_operations vfs_operations_t;
typedef struct dirent dirent_t;

// Directory entry structure
struct dirent {
    char name[128];
    u32 inode;
};

// VFS operations structure
struct vfs_operations {
    int (*read)(vfs_node_t* node, u64 offset, u64 size, u8* buffer);
    int (*write)(vfs_node_t* node, u64 offset, u64 size, const u8* buffer);
    void (*open)(vfs_node_t* node);
    void (*close)(vfs_node_t* node);
    dirent_t* (*readdir)(vfs_node_t* node, u32 index);
    vfs_node_t* (*finddir)(vfs_node_t* node, const char* name);
};

// VFS node structure
struct vfs_node {
    char name[128];         // Node name
    u32 inode;              // Inode number
    u32 type;               // Node type (file, directory, etc.)
    u32 permissions;        // Permission flags
    u32 uid;                // User ID
    u32 gid;                // Group ID
    u64 size;               // Size in bytes
    u64 impl;               // Implementation-specific data
    vfs_operations_t* ops;  // Operations
    vfs_node_t* ptr;        // Used for symlinks or mountpoints
};

// VFS initialization
void vfs_init(void);

// VFS operations
vfs_node_t* vfs_open(const char* path);
void vfs_close(vfs_node_t* node);
int vfs_read(vfs_node_t* node, u64 offset, u64 size, u8* buffer);
int vfs_write(vfs_node_t* node, u64 offset, u64 size, const u8* buffer);
dirent_t* vfs_readdir(vfs_node_t* node, u32 index);
vfs_node_t* vfs_finddir(vfs_node_t* node, const char* name);

// Mount point management
void vfs_mount(const char* path, vfs_node_t* node);
vfs_node_t* vfs_get_root(void);

#endif // VFS_H
