// =============================================================================
// serial_drv — Build Script
// =============================================================================
//
// Tells the linker to use our custom linker script that places the binary
// at 0x400000 (userspace base address).
// =============================================================================

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{}/linker.ld", manifest_dir);
    println!("cargo:rerun-if-changed=linker.ld");
}
