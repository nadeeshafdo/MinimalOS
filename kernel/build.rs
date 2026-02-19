use std::path::Path;

fn main() {
	let build_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("kernel crate must have a parent directory")
		.join("build");
	println!("cargo:rustc-link-search=native={}", build_dir.display());
	println!("cargo:rustc-link-arg=-Tlinker.ld");
	println!("cargo:rerun-if-changed=../build/linker.ld");
}
