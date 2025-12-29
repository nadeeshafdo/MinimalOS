/**
 * MinimalOS - CPU Initialization and Feature Detection
 */

#ifndef ARCH_X86_64_CPU_H
#define ARCH_X86_64_CPU_H

#include <minimalos/types.h>

/* CPUID feature flags (EDX for leaf 1) */
#define CPUID_FEAT_EDX_FPU (1 << 0)
#define CPUID_FEAT_EDX_PSE (1 << 3)
#define CPUID_FEAT_EDX_MSR (1 << 5)
#define CPUID_FEAT_EDX_PAE (1 << 6)
#define CPUID_FEAT_EDX_APIC (1 << 9)
#define CPUID_FEAT_EDX_MTRR (1 << 12)
#define CPUID_FEAT_EDX_PGE (1 << 13)
#define CPUID_FEAT_EDX_FXSR (1 << 24)
#define CPUID_FEAT_EDX_SSE (1 << 25)
#define CPUID_FEAT_EDX_SSE2 (1 << 26)

/* CPUID feature flags (ECX for leaf 1) */
#define CPUID_FEAT_ECX_SSE3 (1 << 0)
#define CPUID_FEAT_ECX_SSE41 (1 << 19)
#define CPUID_FEAT_ECX_SSE42 (1 << 20)
#define CPUID_FEAT_ECX_X2APIC (1 << 21)
#define CPUID_FEAT_ECX_XSAVE (1 << 26)
#define CPUID_FEAT_ECX_AVX (1 << 28)

/* Model Specific Registers */
#define MSR_IA32_APIC_BASE 0x1B
#define MSR_IA32_EFER 0xC0000080
#define MSR_IA32_STAR 0xC0000081
#define MSR_IA32_LSTAR 0xC0000082
#define MSR_IA32_FMASK 0xC0000084
#define MSR_IA32_FS_BASE 0xC0000100
#define MSR_IA32_GS_BASE 0xC0000101
#define MSR_IA32_KERNEL_GS_BASE 0xC0000102

/* CPU information structure */
struct cpu_info {
  char vendor[13];
  char brand[49];
  uint32_t family;
  uint32_t model;
  uint32_t stepping;
  uint32_t features_edx;
  uint32_t features_ecx;
  uint32_t max_cpuid;
  uint32_t max_cpuid_ext;
  uint8_t apic_id;
  bool x2apic_supported;
  bool xsave_supported;
};

/* Global CPU info */
extern struct cpu_info cpu_info;

/* Function prototypes */
void cpu_init(void);
void cpuid(uint32_t leaf, uint32_t *eax, uint32_t *ebx, uint32_t *ecx,
           uint32_t *edx);
uint64_t rdmsr(uint32_t msr);
void wrmsr(uint32_t msr, uint64_t value);
uint64_t read_cr0(void);
uint64_t read_cr2(void);
uint64_t read_cr3(void);
uint64_t read_cr4(void);
void write_cr0(uint64_t value);
void write_cr3(uint64_t value);
void write_cr4(uint64_t value);

/* Inline functions */
static inline void cli(void) { __asm__ volatile("cli"); }

static inline void sti(void) { __asm__ volatile("sti"); }

static inline void hlt(void) { __asm__ volatile("hlt"); }

static inline void pause(void) { __asm__ volatile("pause"); }

static inline void invlpg(void *addr) {
  __asm__ volatile("invlpg (%0)" : : "r"(addr) : "memory");
}

#endif /* ARCH_X86_64_CPU_H */
