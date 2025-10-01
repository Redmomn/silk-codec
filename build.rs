use std::env;
use std::path::PathBuf;

#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64"
))]
static SILK_SDK_PATH: &str = "silk/src/SILK_SDK_SRC_FLP_v1.0.9";

#[cfg(target_arch = "arm")]
static SILK_SDK_PATH: &str = "silk/src/SILK_SDK_SRC_ARM_v1.0.9";

#[cfg(target_arch = "powerpc")]
static SILK_SDK_PATH: &str = "silk/src/SILK_SDK_SRC_FIX_v1.0.9";

fn main() {
    let interface_path = format!("{SILK_SDK_PATH}/interface");
    let src_path = format!("{SILK_SDK_PATH}/src");
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
