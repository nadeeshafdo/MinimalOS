#ifndef INITRD_H
#define INITRD_H

#include "../include/types.h"
#include "vfs.h"

// TAR header structure (USTAR format)
struct tar_header {
    char filename[100];
    char mode[8];
    char uid[8];
    char gid[8];
    char size[12];
    char mtime[12];
    char checksum[8];
    char typeflag;
    char linkname[100];
    char magic[6];
    char version[2];
    char uname[32];
    char gname[32];
    char devmajor[8];
    char devminor[8];
    char prefix[155];
    char padding[12];
} __attribute__((packed));

// Initialize initrd filesystem from TAR archive
vfs_node_t* initrd_init(uintptr tar_address, size_t tar_size);

#endif // INITRD_H
