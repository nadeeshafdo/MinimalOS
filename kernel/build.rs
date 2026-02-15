fn main() {
    println!("cargo:rustc-link-search=native={}", concat!(env!("CARGO_MANIFEST_DIR"), "/../build"));
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    println!("cargo:rerun-if-changed=../build/linker.ld");
}
