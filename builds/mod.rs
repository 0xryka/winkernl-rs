use std::env;
use std::path::Path;

pub mod auto;


pub const ENV_KM_LIB_OVERRIDE: &str = "WINDOWS_KITS_KM_LIB";
pub const DEFAULT_TARGET_ARCH: &str = "x86_64-pc-windows-msvc";
pub const NTOSKRNL_LIB_NAME: &str = "ntoskrnl.lib";


fn link_kernel_libraries(km_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", km_dir.display());
    println!("cargo:rustc-link-lib=static=ntoskrnl");
}


pub fn get_arch() -> &'static str {
    let target = env::var("TARGET").unwrap_or_else(|_| {
        println!("cargo:warning=TARGET environment variable not set, defaulting to {}", DEFAULT_TARGET_ARCH);
        DEFAULT_TARGET_ARCH.to_string()
    });
    get_arch_from_target(&target)
}

fn get_arch_from_target(target: &str) -> &'static str {
    if target.contains("x86_64") {
        "x64"
    } else if target.contains("i686") || target.contains("i586") || target.contains("i486") {
        "x86"
    } else if target.contains("aarch64") {
        "arm64"
    } else {
        println!("cargo:warning=Unrecognized target architecture in '{}', defaulting to x64", target);
        "x64"
    }
}