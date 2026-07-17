use std::{env, fs};
use std::path::{Path, PathBuf};
use crate::builds::*;

pub fn handle_auto_search() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(km_dir_env) = env::var(ENV_KM_LIB_OVERRIDE) {
        let km_path = PathBuf::from(km_dir_env);
        if km_path.is_dir() {
            println!("cargo:warning=Using manually specified Windows Kit directory from {}: {}", ENV_KM_LIB_OVERRIDE, km_path.display());
            link_kernel_libraries(&km_path);
            return Ok(());
        } else {
            println!("cargo:warning={} is set but the path does not exist or is not a directory: {}", ENV_KM_LIB_OVERRIDE, km_path.display());
        }
    }


    let arch = get_arch();

    if !cfg!(target_os = "windows") {
        return Err(format!(
            "Auto-detection of Windows Kits is only supported on Windows hosts.\n\
             Please set the environment variable: {} to your 'km/{}' folder path.",
            ENV_KM_LIB_OVERRIDE, arch
        ).into());
    }

    let kits_root = find_windows_kits_root()?;
    let lib_root = kits_root.join("Lib");

    match find_best_km_dir(&lib_root, arch) {
        Some(km_dir) => {
            link_kernel_libraries(&km_dir);
            Ok(())
        }
        None => Err(format!(
            "Could not find a valid 'km/{}' directory containing '{}' under '{}'.\n\
             Please install the Windows Driver Kit (WDK) or set the {} env var manually.",
            arch, NTOSKRNL_LIB_NAME, lib_root.display(), ENV_KM_LIB_OVERRIDE
        ).into()),
    }
}


fn find_windows_kits_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key_path = r"SOFTWARE\Microsoft\Windows Kits\Installed Roots";
    let key = hklm.open_subkey(key_path)?;
    let kits_root: String = key.get_value("KitsRoot10")?;

    Ok(PathBuf::from(kits_root))
}


fn find_best_km_dir(lib_root: &Path, arch: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(lib_root).ok()?;
    let mut finded = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("10.") {
                let version_parts: Vec<u64> = name.split('.').filter_map(|s| s.parse::<u64>().ok()).collect();
                let km_arch_dir = path.join("km").join(arch);
                if km_arch_dir.is_dir() && km_arch_dir.join(NTOSKRNL_LIB_NAME).is_file() {
                    finded.push((version_parts, km_arch_dir));
                }
            }
        }
    }

    if !finded.is_empty() {
        finded.sort_by(|a, b| a.0.cmp(&b.0));
        let (_, best_path) = finded.pop().unwrap();
        return Some(best_path);
    }

    let entries = fs::read_dir(lib_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let km_arch_dir = path.join("km").join(arch);
        if km_arch_dir.is_dir() && km_arch_dir.join(NTOSKRNL_LIB_NAME).is_file() {
            return Some(km_arch_dir);
        }
    }

    None
}