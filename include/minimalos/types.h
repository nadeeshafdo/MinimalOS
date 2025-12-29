/**
 * MinimalOS - Core Type Definitions
 * Compatible with clangd/IDE language servers
 */

#ifndef MINIMALOS_TYPES_H
#define MINIMALOS_TYPES_H

/* Standard integer types */
typedef signed char int8_t;
typedef unsigned char uint8_t;
typedef signed short int16_t;
typedef unsigned short uint16_t;
typedef signed int int32_t;
typedef unsigned int uint32_t;
typedef signed long long int64_t;
typedef unsigned long long uint64_t;

/* Size types */
typedef uint64_t size_t;
typedef int64_t ssize_t;
typedef int64_t ptrdiff_t;

/* Pointer-sized integer */
typedef uint64_t uintptr_t;
typedef int64_t intptr_t;

/* Boolean - C11 compatible */
#if !defined(__cplusplus)
#if defined(__STDC_VERSION__) && (__STDC_VERSION__ >= 199901L)
typedef _Bool bool;
#else
typedef unsigned char bool;
#endif
#define true 1
#define false 0
#endif

/* NULL pointer */
#ifndef NULL
#define NULL ((void *)0)
#endif

/* Compiler attributes - named to avoid conflicts */
#ifndef __packed
#define __packed __attribute__((packed))
#endif

#ifndef __aligned
#define __aligned(x) __attribute__((aligned(x)))
#endif

#ifndef __always_inline
#define __always_inline __attribute__((always_inline)) inline
#endif

#ifndef __noreturn
#define __noreturn __attribute__((noreturn))
#endif

#ifndef __unused
#define __unused __attribute__((unused))
#endif

/* Bit manipulation */
#define BIT(n) (1UL << (n))
#define ALIGN_UP(x, a) (((x) + (a) - 1) & ~((a) - 1))
#define ALIGN_DOWN(x, a) ((x) & ~((a) - 1))

/* Memory sizes */
#define KB (1024UL)
#define MB (1024UL * KB)
#define GB (1024UL * MB)

/* Page size */
#define PAGE_SIZE 4096UL
#define PAGE_SHIFT 12

/* Kernel virtual address offset */
#define KERNEL_VMA 0xFFFFFFFF80000000UL

/* Physical to virtual and vice versa */
#define PHYS_TO_VIRT(addr) ((void *)((uintptr_t)(addr) + KERNEL_VMA))
#define VIRT_TO_PHYS(addr) ((uintptr_t)(addr) - KERNEL_VMA)

#endif /* MINIMALOS_TYPES_H */
