#ifndef MSR_H
#define MSR_H

#include "../../include/types.h"

// MSR Addresses
#define MSR_EFER     0xC0000080
#define MSR_STAR     0xC0000081
#define MSR_LSTAR    0xC0000082
#define MSR_SFMASK   0xC0000084
#define MSR_GS_BASE  0xC0000101
#define MSR_KERNEL_GS_BASE 0xC0000102

static inline void wrmsr(u32 msr, u64 value) {
    u32 low = value & 0xFFFFFFFF;
    u32 high = value >> 32;
    __asm__ volatile("wrmsr" : : "a"(low), "d"(high), "c"(msr));
}

static inline u64 rdmsr(u32 msr) {
    u32 low, high;
    __asm__ volatile("rdmsr" : "=a"(low), "=d"(high) : "c"(msr));
    return ((u64)high << 32) | low;
}

#endif // MSR_H
