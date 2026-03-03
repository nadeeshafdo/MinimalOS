// =============================================================================
// MinimalOS Kernel — Build Script
// =============================================================================
//
// Passes kernel-specific linker and codegen flags that were previously in
// .cargo/config.toml. These flags are kernel-exclusive and must NOT apply
// to user crates (libmnos, serial_drv) which have different requirements.
//
// Kernel-specific flags:
//   - Linker script: kernel/linker.ld (higher-half mapping at 0xFFFFFFFF80000000)
//   - Code model: kernel (symbols in top 2 GB of virtual address space)
//   - Relocation model: static (no PIC — we know exactly where we're loaded)
// =============================================================================

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let linker_script = format!("{}/linker.ld", manifest_dir);

    // Linker script for higher-half kernel placement
    println!("cargo:rustc-link-arg=-T{}", linker_script);

    // Tell cargo to re-link if the linker script changes
    println!("cargo:rerun-if-changed=linker.ld");
}
