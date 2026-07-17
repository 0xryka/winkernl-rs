//! Kernel module discovery and export resolution.
//!
//! This module provides helpers for querying the list of loaded kernel
//! modules, retrieving their base addresses, and resolving exported
//! routines.
//!
//! It is primarily intended for situations where exported kernel symbols
//! must be located dynamically at runtime instead of being imported
//! statically.
//!
//! # Overview
//!
//! The main helpers provided by this module are:
//!
//! - [`query_system_module`] retrieves information about a loaded kernel
//!   module.
//! - [`get_system_module_base_address`] returns the image base of a loaded
//!   module.
//! - [`find_exported_routine_by_name`] resolves an exported symbol from a
//!   known module base.
//! - [`resolve_system_routine`] combines module lookup and export
//!   resolution into a single operation.
use alloc::string::ToString;
use alloc::vec;
use core::ffi::CStr;
use core::{ptr, slice};

use crate::*;



/// Queries the loaded kernel module list.
///
/// The lookup accepts either the full NT path of a module or its filename
/// only. Comparisons are performed case-insensitively.
///
/// # Arguments
///
/// * `module_name` - Full NT path or module filename.
///
/// # Returns
///
/// Returns the corresponding
/// [`RTL_PROCESS_MODULE_INFORMATION`] if the module is found.
///
/// Returns `Ok(None)` if no matching module exists.
///
/// # Errors
///
/// * [`STATUS_INVALID_PARAMETER`] if `module_name` is empty.
/// * Any error returned by `ZwQuerySystemInformation`.
pub fn query_system_module(module_name: &str) -> Result<Option<RTL_PROCESS_MODULE_INFORMATION>, NTSTATUS> {
    if module_name.is_empty() {
        return Err(STATUS_INVALID_PARAMETER);
    }

    let target = module_name.to_lowercase();
    let mut req_size = 0;

    unsafe {
        let status = ZwQuerySystemInformation(SystemModuleInformation, ptr::null_mut(), 0, &mut req_size);
        if status != STATUS_INFO_LENGTH_MISMATCH && !nt_success(status) {
            return Err(status);
        }
    }

    let mut buffer = vec![0; req_size as usize];

    unsafe {
        let status = ZwQuerySystemInformation(SystemModuleInformation, buffer.as_mut_ptr() as PVOID, req_size, &mut req_size);
        if !nt_success(status) {
            return Err(status);
        }

        let modules = buffer.as_ptr() as *const RTL_PROCESS_MODULES;
        let count = (*modules).NumberOfModules as usize;
        let first = ptr::addr_of!((*modules).Modules) as *const RTL_PROCESS_MODULE_INFORMATION;
        let entries = slice::from_raw_parts(first, count);

        for module in entries {
            let path = &module.FullPathName;

            if let Ok(path) = CStr::from_ptr(path.as_ptr() as *const i8).to_str() {
                let path = path.to_lowercase();

                if path == target {
                    return Ok(Some(*module));
                }

                let offset = module.OffsetToFileName as usize;

                if offset < path.len() {
                    if let Some(file) = path.get(offset..) {
                        if file == target {
                            return Ok(Some(*module));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}



/// Returns the image base of a loaded kernel module.
///
/// This is a convenience wrapper around [`query_system_module`].
///
/// # Arguments
///
/// * `module_name` - Full NT path or module filename.
///
/// # Returns
///
/// Returns the module base address if found.
///
/// Returns `Ok(None)` if the module is not loaded.
///
/// # Errors
///
/// Propagates any error returned by [`query_system_module`].
pub fn get_system_module_base_address(module_name: &str) -> Result<Option<PVOID>, NTSTATUS> {
    query_system_module(module_name).map(|module| module.map(|m|m.ImageBase))
}




/// Resolves an exported routine from a loaded kernel module.
///
/// This function wraps `RtlFindExportedRoutineByName`.
///
/// # Arguments
///
/// * `module_base` - Base address of the module.
/// * `exported_name` - Exported symbol name.
///
/// # Returns
///
/// Returns the address of the exported routine, or `None` if the export
/// does not exist or if the arguments are invalid.
pub fn find_exported_routine_by_name(module_base: PVOID, exported_name: &str) -> Option<PVOID> {
    if module_base.is_null() || exported_name.is_empty() {
        return None;
    }

    let mut name = exported_name.to_string();
    name.push('\0');

    unsafe {
        let addr = RtlFindExportedRoutineByName(module_base, name.as_ptr());
        if addr.is_null() {
            None
        } else {
            Some(addr as PVOID)
        }
    }
}


/// Resolves an exported routine by module and symbol name.
///
/// This helper first locates the requested kernel module, then resolves the
/// specified exported routine.
///
/// # Arguments
///
/// * `module_name` - Module filename or full NT path.
/// * `exported_name` - Exported symbol name.
///
/// # Returns
///
/// Returns the address of the exported routine if both the module and the
/// export exist.
///
/// Returns `Ok(None)` if either the module or the export cannot be found.
///
/// # Errors
///
/// * [`STATUS_INVALID_PARAMETER`] if one of the arguments is empty.
/// * Any error encountered while querying the loaded module list.
pub fn resolve_system_routine(module_name: &str, exported_name: &str) -> Result<Option<PVOID>, NTSTATUS> {
    if module_name.is_empty() || exported_name.is_empty() {
        return Err(STATUS_INVALID_PARAMETER);
    }
    match get_system_module_base_address(module_name)? {
        Some(base) => Ok(find_exported_routine_by_name(base, exported_name)),
        None => Ok(None),
    }
}