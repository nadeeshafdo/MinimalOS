fn main() {
	let build_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("user/display_server must have parent")
		.parent()
		.expect("user must have parent")
		.join("build");
	println!("cargo:rustc-link-search=native={}", build_dir.display());
	println!("cargo:rustc-link-arg=-Tlinker-display_server.ld");
	println!("cargo:rerun-if-changed=../../build/linker-display_server.ld");
}
