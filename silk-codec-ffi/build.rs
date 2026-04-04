use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn target_root_dir() -> PathBuf {
    if let Some(target_dir) = env::var_os("CARGO_TARGET_DIR") {
        let target_dir = PathBuf::from(target_dir);
        if target_dir.is_absolute() {
            return target_dir;
        }
        return PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"))
            .join(target_dir);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    out_dir
        .ancestors()
        .nth(4)
        .map(Path::to_path_buf)
        .expect("failed to determine target dir from OUT_DIR")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/lib.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let config_path = manifest_dir.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path)
        .unwrap_or_else(|err| panic!("failed to load {}: {err}", config_path.display()));

    let header_dir = target_root_dir().join("ffi");
    fs::create_dir_all(&header_dir).expect("failed to create ffi output directory");

    cbindgen::Builder::new()
        .with_crate(manifest_dir)
        .with_config(config)
        .generate()
        .unwrap_or_else(|err| panic!("failed to generate C header: {err}"))
        .write_to_file(header_dir.join("silk_codec.h"));
}
