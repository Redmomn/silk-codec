use std::env;
use std::path::PathBuf;

fn get_silk_sdk_path() -> &'static str {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    match target_arch.as_str() {
        "x86" | "x86_64" | "aarch64" | "powerpc64" => "silk/src/SILK_SDK_SRC_FLP_v1.0.9",
        "arm" => "silk/src/SILK_SDK_SRC_ARM_v1.0.9",
        "powerpc" => "silk/src/SILK_SDK_SRC_FIX_v1.0.9",
        _ => "silk/src/SILK_SDK_SRC_FIX_v1.0.9",
    }
}

fn configure_ffmpeg_static_linking() {
    println!("cargo:rustc-link-lib=c++");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");

    if env::var_os("CARGO_FEATURE_FFMPEG_STATIC").is_some() {
        configure_ffmpeg_static_linking();
    }

    let silk_sdk_path = get_silk_sdk_path();
    let interface_path = format!("{silk_sdk_path}/interface");
    let src_path = format!("{silk_sdk_path}/src");
    let mut files = Vec::new();
    files.extend(
        glob::glob(&format!("{src_path}/*.c"))
            .unwrap()
            .map(|path| path.unwrap().to_path_buf()),
    );
    files.extend(
        glob::glob(&format!("{src_path}/*.S"))
            .unwrap()
            .map(|path| path.unwrap().to_path_buf()),
    );
    cc::Build::new()
        .includes([src_path.as_str(), interface_path.as_str()])
        .files(files)
        .compile("silk");

    println!("cargo:rustc-link-lib=static=silk");

    let bindings = bindgen::Builder::default()
        .header(format!("{interface_path}/SKP_Silk_control.h"))
        .header(format!("{interface_path}/SKP_Silk_errors.h"))
        .header(format!("{interface_path}/SKP_Silk_SDK_API.h"))
        .header(format!("{interface_path}/SKP_Silk_typedef.h"))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("silk_bindings.rs"))
        .expect("Couldn't write bindings!");
}
