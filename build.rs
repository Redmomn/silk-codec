use std::env;
use std::path::PathBuf;

fn get_silk_sdk_path() -> &'static str {
    "silk/src/SILK_SDK_SRC_FIX_v1.0.9"
}

fn main() {
    if env::var_os("CARGO_FEATURE_FFMPEG_STATIC").is_some() {
        println!("cargo:rustc-link-lib=c++");
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

    let bindings = bindgen::Builder::default()
        .header(format!("{interface_path}/SKP_Silk_SDK_API.h"))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("silk_bindings.rs"))
        .expect("Couldn't write bindings!");
}
