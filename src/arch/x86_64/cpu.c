/**
 * MinimalOS - CPU Initialization and Feature Detection
 */

#include "cpu.h"

extern void printk(const char *fmt, ...);

/* Global CPU info */
struct cpu_info cpu_info;

/**
 * Execute CPUID instruction
 */
void cpuid(uint32_t leaf, uint32_t *eax, uint32_t *ebx, uint32_t *ecx,
           uint32_t *edx) {
  __asm__ volatile("cpuid"
                   : "=a"(*eax), "=b"(*ebx), "=c"(*ecx), "=d"(*edx)
                   : "a"(leaf), "c"(0));
}

/**
 * Read Model Specific Register
 */
uint64_t rdmsr(uint32_t msr) {
  uint32_t low, high;
  __asm__ volatile("rdmsr" : "=a"(low), "=d"(high) : "c"(msr));
  return ((uint64_t)high << 32) | low;
}

/**
 * Write Model Specific Register
 */
void wrmsr(uint32_t msr, uint64_t value) {
  uint32_t low = (uint32_t)value;
  uint32_t high = (uint32_t)(value >> 32);
  __asm__ volatile("wrmsr" : : "c"(msr), "a"(low), "d"(high));
}

/**
 * Read control registers
 */
uint64_t read_cr0(void) {
  uint64_t value;
  __asm__ volatile("mov %%cr0, %0" : "=r"(value));
  return value;
}

uint64_t read_cr2(void) {
  uint64_t value;
  __asm__ volatile("mov %%cr2, %0" : "=r"(value));
  return value;
}

uint64_t read_cr3(void) {
  uint64_t value;
  __asm__ volatile("mov %%cr3, %0" : "=r"(value));
  return value;
}

uint64_t read_cr4(void) {
  uint64_t value;
  __asm__ volatile("mov %%cr4, %0" : "=r"(value));
  return value;
}

/**
 * Write control registers
 */
void write_cr0(uint64_t value) {
  __asm__ volatile("mov %0, %%cr0" : : "r"(value) : "memory");
}

void write_cr3(uint64_t value) {
  __asm__ volatile("mov %0, %%cr3" : : "r"(value) : "memory");
}

void write_cr4(uint64_t value) {
  __asm__ volatile("mov %0, %%cr4" : : "r"(value) : "memory");
}

/**
 * Get CPU vendor string
 */
static void get_vendor_string(char *vendor) {
  uint32_t eax, ebx, ecx, edx;
  cpuid(0, &eax, &ebx, &ecx, &edx);

  /* Vendor string is in EBX, EDX, ECX (in that order) */
  *((uint32_t *)&vendor[0]) = ebx;
  *((uint32_t *)&vendor[4]) = edx;
  *((uint32_t *)&vendor[8]) = ecx;
  vendor[12] = '\0';
}

/**
 * Get CPU brand string
 */
static void get_brand_string(char *brand) {
  uint32_t eax, ebx, ecx, edx;

  /* Check if brand string is supported */
  cpuid(0x80000000, &eax, &ebx, &ecx, &edx);
  if (eax < 0x80000004) {
    brand[0] = '\0';
    return;
  }

  /* Get brand string (3 CPUID calls) */
  cpuid(0x80000002, &eax, &ebx, &ecx, &edx);
  *((uint32_t *)&brand[0]) = eax;
  *((uint32_t *)&brand[4]) = ebx;
  *((uint32_t *)&brand[8]) = ecx;
  *((uint32_t *)&brand[12]) = edx;

  cpuid(0x80000003, &eax, &ebx, &ecx, &edx);
  *((uint32_t *)&brand[16]) = eax;
  *((uint32_t *)&brand[20]) = ebx;
  *((uint32_t *)&brand[24]) = ecx;
  *((uint32_t *)&brand[28]) = edx;

  cpuid(0x80000004, &eax, &ebx, &ecx, &edx);
  *((uint32_t *)&brand[32]) = eax;
  *((uint32_t *)&brand[36]) = ebx;
  *((uint32_t *)&brand[40]) = ecx;
  *((uint32_t *)&brand[44]) = edx;
  brand[48] = '\0';
}

/**
 * Initialize CPU - detect features and enable capabilities
 */
void cpu_init(void) {
  uint32_t eax, ebx, ecx, edx;

  /* Get vendor string */
  get_vendor_string(cpu_info.vendor);
  printk("  CPU Vendor: %s\n", cpu_info.vendor);

  /* Get brand string */
  get_brand_string(cpu_info.brand);
  if (cpu_info.brand[0]) {
    printk("  CPU Brand: %s\n", cpu_info.brand);
  }

  /* Get max CPUID leaf */
  cpuid(0, &eax, &ebx, &ecx, &edx);
  cpu_info.max_cpuid = eax;

  /* Get max extended CPUID leaf */
  cpuid(0x80000000, &eax, &ebx, &ecx, &edx);
  cpu_info.max_cpuid_ext = eax;

  /* Get processor info and features (leaf 1) */
  cpuid(1, &eax, &ebx, &ecx, &edx);

  cpu_info.stepping = eax & 0xF;
  cpu_info.model = (eax >> 4) & 0xF;
  cpu_info.family = (eax >> 8) & 0xF;

  /* Extended model/family for newer CPUs */
  if (cpu_info.family == 0xF) {
    cpu_info.family += (eax >> 20) & 0xFF;
  }
  if (cpu_info.family == 0x6 || cpu_info.family == 0xF) {
    cpu_info.model += ((eax >> 16) & 0xF) << 4;
  }

  cpu_info.features_edx = edx;
  cpu_info.features_ecx = ecx;
  cpu_info.apic_id = (ebx >> 24) & 0xFF;

  printk("  Family: %u, Model: %u, Stepping: %u\n", cpu_info.family,
         cpu_info.model, cpu_info.stepping);
  printk("  APIC ID: %u\n", cpu_info.apic_id);

  /* Check for key features */
  cpu_info.x2apic_supported = (ecx & CPUID_FEAT_ECX_X2APIC) != 0;
  cpu_info.xsave_supported = (ecx & CPUID_FEAT_ECX_XSAVE) != 0;

  printk("  Features: ");
  if (edx & CPUID_FEAT_EDX_FPU)
    printk("FPU ");
  if (edx & CPUID_FEAT_EDX_PAE)
    printk("PAE ");
  if (edx & CPUID_FEAT_EDX_APIC)
    printk("APIC ");
  if (edx & CPUID_FEAT_EDX_FXSR)
    printk("FXSR ");
  if (edx & CPUID_FEAT_EDX_SSE)
    printk("SSE ");
  if (edx & CPUID_FEAT_EDX_SSE2)
    printk("SSE2 ");
  if (ecx & CPUID_FEAT_ECX_SSE3)
    printk("SSE3 ");
  if (ecx & CPUID_FEAT_ECX_X2APIC)
    printk("x2APIC ");
  if (ecx & CPUID_FEAT_ECX_XSAVE)
    printk("XSAVE ");
  if (ecx & CPUID_FEAT_ECX_AVX)
    printk("AVX ");
  printk("\n");

  /* Enable SSE/SSE2 (required for 64-bit mode) */
  uint64_t cr0 = read_cr0();
  cr0 &= ~(1UL << 2); /* Clear EM (emulation) */
  cr0 |= (1UL << 1);  /* Set MP (monitor coprocessor) */
  write_cr0(cr0);

  uint64_t cr4 = read_cr4();
  cr4 |= (1UL << 9);  /* Set OSFXSR */
  cr4 |= (1UL << 10); /* Set OSXMMEXCPT */

  /* Enable XSAVE if supported */
  if (cpu_info.xsave_supported) {
    cr4 |= (1UL << 18); /* Set OSXSAVE */
  }

  write_cr4(cr4);

  printk("  SSE/FXSR enabled\n");
}
