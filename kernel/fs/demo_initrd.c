/* Built-in demo initrd data */

#include <stdint.h>
#include "initrd.h"

/* Demo file contents */
static const char readme_txt[] = 
    "Welcome to MinimalOS!\n"
    "\n"
    "This is a 64-bit operating system written from scratch.\n"
    "Features:\n"
    "  - 64-bit long mode\n"
    "  - Memory management\n"
    "  - Process management\n"
    "  - System calls\n"
    "  - Virtual file system\n";

static const char hello_txt[] = 
    "Hello, World!\n"
    "This file is stored in the initrd.\n";

static const char version_txt[] = 
    "MinimalOS v0.1.0\n"
    "Build: 64-bit x86_64\n";

/* Create the initrd image in memory */
static uint8_t initrd_image[2048];
static int initrd_built = 0;

/* String length helper */
static uint32_t demo_strlen(const char *s) {
    uint32_t len = 0;
    while (s[len]) len++;
    return len;
}

/* String copy helper */
static void demo_strcpy(char *dst, const char *src) {
    while (*src) *dst++ = *src++;
    *dst = '\0';
}

/* Memory copy helper */
static void demo_memcpy(void *dst, const void *src, uint32_t n) {
    uint8_t *d = (uint8_t *)dst;
    const uint8_t *s = (const uint8_t *)src;
    while (n--) *d++ = *s++;
}

uint64_t demo_initrd_build(void) {
    if (initrd_built) return (uint64_t)initrd_image;
    
    uint8_t *ptr = initrd_image;
    
    /* Header */
    initrd_header_t *header = (initrd_header_t *)ptr;
    header->magic = INITRD_MAGIC;
    header->num_files = 3;
    ptr += sizeof(initrd_header_t);
    
    /* File 1: readme.txt */
    initrd_file_header_t *f1 = (initrd_file_header_t *)ptr;
    demo_strcpy(f1->name, "readme.txt");
    f1->size = demo_strlen(readme_txt);
    ptr += sizeof(initrd_file_header_t);
    demo_memcpy(ptr, readme_txt, f1->size);
    ptr += f1->size;
    
    /* File 2: hello.txt */
    initrd_file_header_t *f2 = (initrd_file_header_t *)ptr;
    demo_strcpy(f2->name, "hello.txt");
    f2->size = demo_strlen(hello_txt);
    ptr += sizeof(initrd_file_header_t);
    demo_memcpy(ptr, hello_txt, f2->size);
    ptr += f2->size;
    
    /* File 3: version.txt */
    initrd_file_header_t *f3 = (initrd_file_header_t *)ptr;
    demo_strcpy(f3->name, "version.txt");
    f3->size = demo_strlen(version_txt);
    ptr += sizeof(initrd_file_header_t);
    demo_memcpy(ptr, version_txt, f3->size);
    ptr += f3->size;
    
    initrd_built = 1;
    return (uint64_t)initrd_image;
}
