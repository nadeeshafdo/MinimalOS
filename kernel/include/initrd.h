#ifndef _INITRD_H
#define _INITRD_H

#include <stdint.h>
#include "vfs.h"

/* Maximum files in initrd */
#define INITRD_MAX_FILES 32

/* Initrd file header (in image) */
typedef struct {
    char name[60];
    uint32_t size;
} __attribute__((packed)) initrd_file_header_t;

/* Initrd header (at start of image) */
typedef struct {
    uint32_t magic;         /* 0x12345678 */
    uint32_t num_files;
} __attribute__((packed)) initrd_header_t;

#define INITRD_MAGIC 0x12345678

/* Initialize initrd from memory location */
vfs_node_t *initrd_init(uint64_t location);

#endif /* _INITRD_H */
