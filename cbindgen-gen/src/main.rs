use std::path::PathBuf;

fn main() {
    // Resolve paths relative to the workspace root (the parent of this crate),
    // independent of the current working directory.
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cbindgen-gen should live inside the workspace")
        .to_path_buf();
    let ffi = workspace.join("ooniprobe-ffi");

    let src = ffi.join("src").join("capi.rs");
    let config = cbindgen::Config::from_file(ffi.join("cbindgen.toml")).unwrap_or_default();
    let output = ffi.join("ooniprobe_userauth.h");

    cbindgen::Builder::new()
        .with_src(&src)
        .with_config(config)
        .generate()
        .expect("failed to generate C header")
        .write_to_file(&output);

    println!("generated {}", output.display());
}
