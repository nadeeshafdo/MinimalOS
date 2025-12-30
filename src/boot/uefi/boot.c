/**
 * MinimalOS - UEFI Bootloader
 * Loads the kernel and transitions to 64-bit mode
 */

#include <efi.h>
#include <efilib.h>

/* Kernel entry point */
typedef void (*kernel_entry_t)(uint64_t boot_info_addr);

/* Page table constants */
#define PAGE_PRESENT (1ULL << 0)
#define PAGE_WRITABLE (1ULL << 1)
#define PAGE_HUGE (1ULL << 7)
#define KERNEL_VMA 0xFFFFFFFF80000000ULL

/* Boot info structure (simplified, passed to kernel) */
struct uefi_boot_info {
  UINT32 magic;
  UINT32 boot_type;
  UINT32 memory_map_entries;
  UINT32 reserved;
  UINT64 memory_map;
  UINT64 framebuffer_addr;
  UINT32 framebuffer_width;
  UINT32 framebuffer_height;
  UINT32 framebuffer_pitch;
  UINT32 framebuffer_bpp;
  UINT64 rsdp_addr;
  UINT64 kernel_phys_start;
  UINT64 kernel_phys_end;
};

#define BOOTINFO_MAGIC 0x4D494E4F

/* Page tables for higher-half mapping */
static UINT64 *pml4 = NULL;
static UINT64 *pdpt = NULL;
static UINT64 *pd = NULL;

/**
 * Setup page tables for higher-half kernel
 */
static EFI_STATUS setup_page_tables(EFI_SYSTEM_TABLE *systab) {
  EFI_STATUS status;

  /* Allocate page tables */
  status = uefi_call_wrapper(systab->BootServices->AllocatePages, 4,
                             AllocateAnyPages, EfiLoaderData, 3,
                             (EFI_PHYSICAL_ADDRESS *)&pml4);
  if (EFI_ERROR(status))
    return status;

  pdpt = pml4 + 512;
  pd = pdpt + 512;

  /* Zero out tables */
  SetMem(pml4, 4096 * 3, 0);

  /* Identity map first 1GB (for boot code) */
  pml4[0] = ((UINT64)pdpt) | PAGE_PRESENT | PAGE_WRITABLE;
  pdpt[0] = ((UINT64)pd) | PAGE_PRESENT | PAGE_WRITABLE;

  /* Map first 1GB using 2MB huge pages */
  for (int i = 0; i < 512; i++) {
    pd[i] = (i * 0x200000ULL) | PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE;
  }

  /* Higher-half mapping at -2GB (0xFFFFFFFF80000000) */
  /* PML4[511] -> PDPT_high */
  /* PDPT_high[510] -> PD (same as above, maps at -2GB) */
  pml4[511] = ((UINT64)pdpt) | PAGE_PRESENT | PAGE_WRITABLE;
  pdpt[510] = ((UINT64)pd) | PAGE_PRESENT | PAGE_WRITABLE;

  return EFI_SUCCESS;
}

/**
 * Find RSDP (ACPI Root System Description Pointer)
 */
static UINT64 find_rsdp(EFI_SYSTEM_TABLE *systab) {
  EFI_GUID acpi_guid = ACPI_20_TABLE_GUID;
  EFI_GUID acpi10_guid = ACPI_TABLE_GUID;

  for (UINTN i = 0; i < systab->NumberOfTableEntries; i++) {
    if (CompareGuid(&systab->ConfigurationTable[i].VendorGuid, &acpi_guid) ||
        CompareGuid(&systab->ConfigurationTable[i].VendorGuid, &acpi10_guid)) {
      return (UINT64)systab->ConfigurationTable[i].VendorTable;
    }
  }
  return 0;
}

/**
 * EFI entry point
 */
EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *systab) {
  EFI_STATUS status;
  EFI_LOADED_IMAGE *loaded_image;
  EFI_GRAPHICS_OUTPUT_PROTOCOL *gop;
  UINTN map_size = 0, map_key, desc_size;
  UINT32 desc_version;
  EFI_MEMORY_DESCRIPTOR *memory_map = NULL;
  struct uefi_boot_info *boot_info;

  /* Initialize UEFI library */
  InitializeLib(image, systab);

  Print(L"MinimalOS UEFI Bootloader\n");
  Print(L"=========================\n\n");

  /* Get loaded image info */
  status =
      uefi_call_wrapper(systab->BootServices->HandleProtocol, 3, image,
                        &gEfiLoadedImageProtocolGuid, (void **)&loaded_image);
  if (EFI_ERROR(status)) {
    Print(L"Error: Failed to get loaded image\n");
    return status;
  }

  /* Get GOP for framebuffer */
  status =
      uefi_call_wrapper(systab->BootServices->LocateProtocol, 3,
                        &gEfiGraphicsOutputProtocolGuid, NULL, (void **)&gop);
  if (EFI_ERROR(status)) {
    Print(L"Warning: No GOP, text mode only\n");
    gop = NULL;
  }

  /* Allocate boot info structure */
  status =
      uefi_call_wrapper(systab->BootServices->AllocatePool, 3, EfiLoaderData,
                        sizeof(struct uefi_boot_info), (void **)&boot_info);
  if (EFI_ERROR(status)) {
    Print(L"Error: Failed to allocate boot info\n");
    return status;
  }
  SetMem(boot_info, sizeof(struct uefi_boot_info), 0);

  boot_info->magic = BOOTINFO_MAGIC;
  boot_info->boot_type = 1; /* UEFI */

  /* Fill framebuffer info */
  if (gop) {
    boot_info->framebuffer_addr = gop->Mode->FrameBufferBase;
    boot_info->framebuffer_width = gop->Mode->Info->HorizontalResolution;
    boot_info->framebuffer_height = gop->Mode->Info->VerticalResolution;
    boot_info->framebuffer_pitch = gop->Mode->Info->PixelsPerScanLine * 4;
    boot_info->framebuffer_bpp = 32;

    Print(L"Framebuffer: %dx%d @ 0x%lx\n", boot_info->framebuffer_width,
          boot_info->framebuffer_height, boot_info->framebuffer_addr);
  }

  /* Find RSDP */
  boot_info->rsdp_addr = find_rsdp(systab);
  Print(L"RSDP: 0x%lx\n", boot_info->rsdp_addr);

  /* Setup page tables */
  Print(L"Setting up page tables...\n");
  status = setup_page_tables(systab);
  if (EFI_ERROR(status)) {
    Print(L"Error: Page table setup failed\n");
    return status;
  }

  /* Get memory map size */
  status = uefi_call_wrapper(systab->BootServices->GetMemoryMap, 5, &map_size,
                             NULL, &map_key, &desc_size, &desc_version);
  map_size += 2 * desc_size; /* Extra space */

  /* Allocate memory for map */
  status = uefi_call_wrapper(systab->BootServices->AllocatePool, 3,
                             EfiLoaderData, map_size, (void **)&memory_map);
  if (EFI_ERROR(status)) {
    Print(L"Error: Failed to allocate memory map\n");
    return status;
  }

  /* Get actual memory map */
  status = uefi_call_wrapper(systab->BootServices->GetMemoryMap, 5, &map_size,
                             memory_map, &map_key, &desc_size, &desc_version);
  if (EFI_ERROR(status)) {
    Print(L"Error: Failed to get memory map\n");
    return status;
  }

  boot_info->memory_map = (UINT64)memory_map;
  boot_info->memory_map_entries = map_size / desc_size;

  Print(L"Memory map: %d entries\n", boot_info->memory_map_entries);

  /* For now, just print success and halt */
  /* TODO: Load kernel.elf from filesystem */
  /* TODO: Exit boot services */
  /* TODO: Jump to kernel */

  Print(L"\nUEFI boot ready. Kernel loading not yet implemented.\n");
  Print(L"Press any key to continue...\n");

  /* Wait for key */
  uefi_call_wrapper(systab->ConIn->Reset, 2, systab->ConIn, FALSE);
  EFI_INPUT_KEY key;
  while (uefi_call_wrapper(systab->ConIn->ReadKeyStroke, 2, systab->ConIn,
                           &key) == EFI_NOT_READY)
    ;

  return EFI_SUCCESS;
}
