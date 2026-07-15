use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=builds.rs");

    let wdk_km_path = r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.22621.0\km";
    let wdk_shared_path = r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared";

    let bindings = bindgen::Builder::default()
        .raw_line("#![allow(warnings)]")
        .raw_line("#![allow(clippy::all)]")
        .header(format!("{}/ntifs.h", wdk_km_path))
        .header(format!("{}/ntddk.h", wdk_km_path))
        .header(format!("{}/ntstatus.h", wdk_shared_path))
        .clang_arg("--target=x86_64-pc-windows-msvc")
        .clang_arg(format!("-I{}", wdk_km_path))
        .clang_arg(format!("-I{}", wdk_shared_path))
        .clang_arg("-D_AMD64_")
        .clang_arg("-D_KERNEL_MODE")
        .clang_arg("-fms-extensions")
        .clang_arg("-DUMDF_USING_NTSTATUS")
        .use_core()
        .derive_default(true)
        .default_enum_style(bindgen::EnumVariation::Rust { non_exhaustive: true })
        .derive_copy(true)
        .layout_tests(false)
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src").join("sys");
    bindings.write_to_file(out_path).unwrap();
}