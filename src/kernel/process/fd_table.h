#ifndef FD_TABLE_H
#define FD_TABLE_H

#include "../include/types.h"
#include "../fs/vfs.h"

#define MAX_FDS 64

// File descriptor structure
typedef struct {
    vfs_node_t* node;
    u64 position;
    u32 flags;
    bool in_use;
} file_descriptor_t;

// File descriptor table (per process)
typedef struct {
    file_descriptor_t fds[MAX_FDS];
} fd_table_t;

// Initialize file descriptor table
void fd_table_init(fd_table_t* table);

// Allocate a file descriptor
int fd_alloc(fd_table_t* table, vfs_node_t* node, u32 flags);

// Free a file descriptor
void fd_free(fd_table_t* table, int fd);

// Get file descriptor
file_descriptor_t* fd_get(fd_table_t* table, int fd);

// Standard file descriptors
#define STDIN  0
#define STDOUT 1
#define STDERR 2

// File open flags
#define O_RDONLY 0x0000
#define O_WRONLY 0x0001
#define O_RDWR   0x0002
#define O_CREAT  0x0040
#define O_TRUNC  0x0200
#define O_APPEND 0x0400

#endif // FD_TABLE_H
