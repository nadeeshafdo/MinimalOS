#ifndef ELF_H
#define ELF_H

#include "../include/types.h"

// ELF Magic number
#define ELF_MAGIC 0x464C457F  // "\x7fELF"

// ELF Class
#define ELFCLASS32 1
#define ELFCLASS64 2

// ELF Data encoding
#define ELFDATA2LSB 1  // Little endian
#define ELFDATA2MSB 2  // Big endian

// ELF Type
#define ET_NONE   0    // No file type
#define ET_REL    1    // Relocatable file
#define ET_EXEC   2    // Executable file
#define ET_DYN    3    // Shared object file
#define ET_CORE   4    // Core file

// ELF Machine
#define EM_X86_64 62   // AMD x86-64

// Program header types
#define PT_NULL    0   // Unused entry
#define PT_LOAD    1   // Loadable segment
#define PT_DYNAMIC 2   // Dynamic linking information
#define PT_INTERP  3   // Interpreter path
#define PT_NOTE    4   // Auxiliary information
#define PT_SHLIB   5   // Reserved
#define PT_PHDR    6   // Program header table

// Program header flags
#define PF_X 0x1       // Execute
#define PF_W 0x2       // Write
#define PF_R 0x4       // Read

// ELF64 Header (64 bytes)
typedef struct {
    u8  ident[16];     // Magic number and other info
    u16 type;          // Object file type
    u16 machine;       // Architecture
    u32 version;       // Object file version
    u64 entry;         // Entry point virtual address
    u64 phoff;         // Program header table file offset
    u64 shoff;         // Section header table file offset
    u32 flags;         // Processor-specific flags
    u16 ehsize;        // ELF header size in bytes
    u16 phentsize;     // Program header table entry size
    u16 phnum;         // Program header table entry count
    u16 shentsize;     // Section header table entry size
    u16 shnum;         // Section header table entry count
    u16 shstrndx;      // Section header string table index
} __attribute__((packed)) elf64_header_t;

// ELF64 Program Header (56 bytes)
typedef struct {
    u32 type;          // Segment type
    u32 flags;         // Segment flags
    u64 offset;        // Segment file offset
    u64 vaddr;         // Segment virtual address
    u64 paddr;         // Segment physical address
    u64 filesz;        // Segment size in file
    u64 memsz;         // Segment size in memory
    u64 align;         // Segment alignment
} __attribute__((packed)) elf64_program_header_t;

// Forward declaration
struct process;

/**
 * Validate ELF binary
 */
bool elf_validate(const void* elf_data);

/**
 * Load ELF binary into process
 */
int elf_load(struct process* proc, const void* elf_data, size_t size);

/**
 * Get entry point from ELF
 */
u64 elf_get_entry(const void* elf_data);

#endif // ELF_H
