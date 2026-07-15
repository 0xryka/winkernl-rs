//! Process management utilities.
//!
//! This module provides helpers for locating and manipulating Windows
//! kernel process objects [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS).
//!
//! Unlike the standard Windows kernel APIs, these utilities operate
//! directly on internal kernel structures, allowing process information
//! to be queried even when no exported routine exists.
//!
//! # Overview
//!
//! The module provides:
//!
//! - Helpers to locate the kernel's initial system process.
//! - Process enumeration by walking the `ActiveProcessLinks` list.
//! - Low-level accessors implemented in the [`eprocess`] module.
//!
//! The [`eprocess`] submodule is responsible for discovering offsets of
//! fields within the [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS) structure and provides helpers for
//! reading or modifying those fields without relying on the standard
//! Windows process APIs.
//!
//! Since [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS) is an undocumented structure, discovered offsets may
//! differ between Windows versions.
use core::sync::atomic::{AtomicU64, Ordering};
use crate::{memory::module::resolve_system_routine, LIST_ENTRY, PEPROCESS};
use crate::process::eprocess::get_active_process_links_offset;
pub mod eprocess;


/// Errors that may occur while locating kernel process information.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SearchError {
    UnknownModule,
    UnknownFunction,
    UnknownOffset,
}


/// Cached address of the exported `PsInitialSystemProcess` object.
///
/// The value is resolved only once and reused by subsequent calls to
/// [`get_initial_system_process`].
pub static PS_INITIAL_SYSTEM_PROCESS_CACHE: AtomicU64 = AtomicU64::new(0);


/// Returns the kernel's initial system process (`System`).
///
/// The exported `PsInitialSystemProcess` symbol is resolved from
/// `ntoskrnl.exe` on the first call and its address is cached for all
/// subsequent invocations.
///
/// # Errors
///
/// Returns [`SearchError`] if the kernel module, exported symbol or
/// cached address cannot be obtained.
pub fn get_initial_system_process() -> Result<PEPROCESS, SearchError> {
    let cached = PS_INITIAL_SYSTEM_PROCESS_CACHE.load(Ordering::Relaxed);
    if cached != 0 {
        return Ok(cached as _);
    }
    let exported = resolve_system_routine(r"\SystemRoot\System32\ntoskrnl.exe", "PsInitialSystemProcess").map_err(|_| SearchError::UnknownModule)?;
    let exported = exported.ok_or(SearchError::UnknownFunction)?;
    let system_process = unsafe { *(exported as *const PEPROCESS) };
    PS_INITIAL_SYSTEM_PROCESS_CACHE.store(system_process as u64, Ordering::Relaxed);
    Ok(system_process)
}



/// Searches for a process by its process identifier.
///
/// This function walks the kernel's `ActiveProcessLinks` list starting
/// from `PsInitialSystemProcess` until a matching process identifier is
/// found.
///
/// Unlike `PsLookupProcessByProcessId`, this function performs a manual
/// traversal of the `EPROCESS` list and therefore depends on the
/// correctness of the discovered `EPROCESS` field offsets.
///
/// # Arguments
///
/// * `pid` - Process identifier to search for.
///
/// # Returns
///
/// Returns `Ok(Some(process))` if a matching `EPROCESS` is found,
/// `Ok(None)` if no process with the specified identifier exists, or
/// `Err(SearchError)` if the required kernel information could not be
/// resolved.
pub fn get_process_by_id(pid: u32) -> Result<Option<PEPROCESS>, SearchError> {
    let system_process = get_initial_system_process()?;
    let pid_offset = eprocess::search_process_id_offset()?.ok_or(SearchError::UnknownOffset)? as usize;
    let link_offset = get_active_process_links_offset();
    let mut process = system_process as *mut u8;
    loop {
        unsafe {
            let current_pid = *(process.add(pid_offset) as *const usize);
            if current_pid == pid as usize {
                return Ok(Some(process as _));
            }
            let active_process_links = process.add(link_offset) as *const LIST_ENTRY;
            let flink = (*active_process_links).Flink;
            if flink.is_null() {
                return Ok(None);
            }
            process = (flink as *mut u8).sub(link_offset);
            if process == system_process as *mut u8 {
                return Ok(None);
            }
        }
    }
}