//! Virtual memory read/write utilities.
//!
//! This module provides high-level helpers for accessing virtual memory
//! both in the current kernel address space and in user processes.
//!
//! # Overview
//!
//! The provided helpers wrap common Windows kernel memory management
//! routines:
//!
//! - [`write_to_read_only_memory`] temporarily remaps a read-only region
//!   through an MDL in order to modify its contents.
//! - [`mm_read_virtual_memory_from_pid`] reads memory from another process
//!   using `MmCopyVirtualMemory`.
//! - [`mm_write_virtual_memory_from_pid`] writes memory into another
//!   process using `MmCopyVirtualMemory`.
//!
//! These helpers automatically manage temporary kernel resources such as
//! MDLs and process object references.
use core::ptr;
use x86_64::VirtAddr;
use crate::*;
use crate::memory::mdl::Mdl;



/// Writes data into a read-only virtual memory region.
///
/// This function creates an MDL describing the destination range,
/// locks the underlying pages, maps them into the system address space,
/// changes the mapping protection to writable and copies the supplied
/// bytes.
///
/// All temporary resources are automatically released before the
/// function returns.
///
/// # Arguments
///
/// - `dst` - Destination virtual address.
/// - `src` - Bytes to write.
///
/// # Returns
///
/// Returns `true` if the write completed successfully, or `false` fif any step of the operation failed.
pub fn write_to_read_only_memory(dst: VirtAddr, src: &[u8]) -> bool {
    unsafe {
        if dst.is_null() || src.is_empty() {
            return false;
        }
        let mut mdl = match Mdl::new(dst, src.len()) {
            Some(mdl) => mdl,
            None => return false,
        };
        mdl.lock(0, LOCK_OPERATION::IoWriteAccess);
        let mut mapping = match mdl.map_locked::<u8>(0, MEMORY_CACHING_TYPE::MmNonCached, None, false, MM_PAGE_PRIORITY::NormalPagePriority) {
            Some(mapping) => mapping,
            None => return false,
        };
        mapping.protect(PAGE_READWRITE);
        ptr::copy_nonoverlapping(src.as_ptr(), mapping.as_mut_ptr(), src.len());
        true
    }
}


/// Reads virtual memory from another process.
///
/// This is a convenience wrapper around `MmCopyVirtualMemory`.
///
/// The target process is looked up from its process identifier and its
/// object reference is automatically released before returning.
///
/// # Arguments
///
/// - `pid` - Identifier of the source process.
/// - `addr` - Virtual address to read.
/// - `out` - Destination buffer.
///
/// # Errors
///
/// Returns the underlying `NTSTATUS` if the process lookup or memory
/// copy operation fails.
///
/// Returns:
///
/// - `STATUS_UNSUCCESSFUL` if no bytes were copied.
/// - `STATUS_BUFFER_TOO_SMALL` if only part of the requested buffer
///   could be copied.
pub fn mm_read_virtual_memory_from_pid(pid: u64, addr: u64, out: &mut [u8]) -> Result<(), NTSTATUS> {
    unsafe {
        let mut src_process: PEPROCESS = ptr::null_mut();
        let status = PsLookupProcessByProcessId(pid as PVOID, &mut src_process);
        if !nt_success(status) {
            return Err(status);
        }
        let mut bytes_copied = 0;
        let status = MmCopyVirtualMemory(src_process, addr as _, PsGetCurrentProcess(), out.as_mut_ptr() as _, out.len() as _, 0, &mut bytes_copied);
        ObfDereferenceObject(src_process as _);
        if !nt_success(status) {
            return Err(status);
        }
        if bytes_copied == 0 {
            return Err(STATUS_UNSUCCESSFUL);
        }
        if bytes_copied != out.len() as _ {
            return Err(STATUS_BUFFER_TOO_SMALL);
        }
        Ok(())
    }
}



/// Writes virtual memory into another process.
///
/// This function copies the contents of `buffer` into the target process
/// using `MmCopyVirtualMemory`.
///
/// The target process is automatically referenced and dereferenced for
/// the duration of the operation.
///
/// # Arguments
///
/// - `pid` - Identifier of the destination process.
/// - `addr` - Destination virtual address.
/// - `buffer` - Data to write.
///
/// # Errors
///
/// Returns the underlying `NTSTATUS` if the process lookup or memory
/// copy operation fails.
///
/// Returns:
///
/// - `STATUS_UNSUCCESSFUL` if no bytes were written.
/// - `STATUS_BUFFER_TOO_SMALL` if only part of the supplied buffer was
///   copied.
pub fn mm_write_virtual_memory_from_pid(pid: u64, addr: u64, buffer: &[u8]) -> Result<(), NTSTATUS> {
    unsafe {
        let mut target_process: PEPROCESS = ptr::null_mut();
        let status = PsLookupProcessByProcessId(pid as PVOID, &mut target_process);
        if !nt_success(status) {
            return Err(status);
        }
        let mut copied = 0;
        let status = MmCopyVirtualMemory(PsGetCurrentProcess(), buffer.as_ptr() as _, target_process, addr as _, buffer.len() as _, 0, &mut copied);
        ObfDereferenceObject(target_process as _);
        if !nt_success(status) {
            return Err(status);
        }
        if copied == 0 {
            return Err(STATUS_UNSUCCESSFUL);
        }
        if copied != buffer.len() as _ {
            return Err(STATUS_BUFFER_TOO_SMALL);
        }
        Ok(())
    }
}