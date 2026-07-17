//! Dynamic discovery of [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS) structure offsets.
//!
//! Windows does not guarantee stable structure layouts across releases.
//! Instead of relying on hardcoded offsets, this module recovers them at
//! runtime by analysing exported kernel helper routines.
//!
//! The recovered offsets are cached after the first successful lookup and
//! reused for the lifetime of the driver.

use alloc::vec;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};
use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind, Register};
use crate::memory::module::resolve_system_routine;
use crate::{RtlGetVersion, PAGE_SIZE, PEPROCESS, RTL_OSVERSIONINFOW};
use crate::process::SearchError;

/// Returns the offset of the `ActiveProcessLinks` member within the
/// [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS) structure for the current Windows builds.
///
/// Unlike the other [`EPROCESS`](https://www.vergiliusproject.com/kernels/x64/windows-11/25h2/_EPROCESS) fields exposed by this module, the
/// `ActiveProcessLinks` offset is currently determined using a
/// builds-specific lookup table.
///
/// # Returns
///
/// Returns the byte offset of the `ActiveProcessLinks` member.
///
/// # Notes
///
/// This function relies on known offsets for supported Windows builds
/// and should be updated as new Windows versions are released.
///
/// The currently supported offsets are:
///
/// | Windows builds | Offset |
/// |---------------|-------:|
/// | 26100 (24H2)  | `0x1D8` |
/// | 26200 (24H2+) | `0x1D8` |
/// | Other builds  | `0x448` |
pub fn get_active_process_links_offset() -> usize {
    unsafe {
        let mut os_version = RTL_OSVERSIONINFOW::default();
        os_version.dwOSVersionInfoSize = size_of::<RTL_OSVERSIONINFOW>() as u32;
        RtlGetVersion(&mut os_version);
        match os_version.dwBuildNumber {
            26100 | 26200 => 0x1D8,
            _ => 0x448,
        }
    }
}



/// Reads a field from an `EPROCESS` structure.
///
/// The field offset is obtained from the supplied offset discovery
/// routine, then used to read the requested value directly from the
/// process object.
///
/// # Arguments
///
/// * `eprocess` - Pointer to the target `EPROCESS`.
/// * `offset_fn` - Function responsible for locating the field offset.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(value))` if the field was successfully read.
/// - `Ok(None)` if the field offset could not be recovered.
/// - `Err(SearchError)` if an error occurred while searching for the
///   offset.
fn read_eprocess_field<F: FnOnce() -> Result<Option<u64>, SearchError>, T: Copy>(eprocess: PEPROCESS, offset_fn: F) -> Result<Option<T>, SearchError> {
    offset_fn().map(|offset| {
        offset.map(|offset| unsafe {
            *((eprocess as *const u8).add(offset as usize) as *const T)
        })
    })
}




/// Searches for a structure field offset and caches the result.
///
/// The supplied cache is checked before attempting any lookup. If the
/// offset has already been discovered, the cached value is returned
/// immediately.
///
/// Otherwise, the exported routine is resolved, analysed, and the
/// recovered offset is stored for future calls.
///
/// # Arguments
///
/// * `cache` - Atomic cache used to store the recovered offset.
/// * `module_name` - Module exporting the accessor routine.
/// * `routine_name` - Name of the exported routine.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(offset))` if the field offset was recovered.
/// - `Ok(None)` if the routine does not expose the expected accessor
///   pattern.
/// - `Err(SearchError)` if the exported routine could not be resolved.
fn search_cached_offset(cache: &AtomicU64, module_name: &str, routine_name: &str) -> Result<Option<u64>, SearchError> {
    let cached_offset = cache.load(Ordering::Relaxed);
    if cached_offset != 0 {
        return Ok(Some(cached_offset));
    }
    let offset = find_offset_from_routine(module_name, routine_name)?;
    if let Some(offset) = offset {
        cache.store(offset, Ordering::Relaxed);
    }
    Ok(offset)
}


/// Resolves an exported kernel routine and attempts to recover the
/// structure field offset it accesses.
///
/// This is a convenience wrapper around [`resolve_system_routine`] and
/// [`find_structure_offset`].
///
/// # Arguments
///
/// * `module_name` - Name or NT path of the module exporting the routine.
/// * `routine_name` - Name of the exported routine.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(offset))` if the field offset was successfully recovered.
/// - `Ok(None)` if the routine does not match the expected accessor
///   pattern.
/// - `Err(SearchError::UnknownFunction)` if the exported routine could
///   not be resolved.
pub fn find_offset_from_routine(module_name: &str, routine_name: &str) -> Result<Option<u64>, SearchError> {
    let routine = resolve_system_routine(module_name, routine_name).map_err(|_| SearchError::UnknownFunction)?.ok_or(SearchError::UnknownFunction)?;
    Ok(find_structure_offset(routine as u64))
}



/// Attempts to recover a structure field offset from a kernel accessor.
///
/// The target routine is copied into a temporary buffer before being
/// decoded using `iced-x86`.
///
/// Simple Windows kernel accessors are commonly implemented as:
///
/// ```text
/// mov rax, [rcx + offset]
/// ret
/// ```
///
/// where `RCX` points to the target structure. The displacement used by
/// the memory operand corresponds to the field offset.
///
/// The decoder scans the routine until either the expected instruction
/// is found or a `ret` instruction is encountered.
///
/// # Arguments
///
/// * `function_address` - Virtual address of the routine to analyse.
///
/// # Returns
///
/// Returns the recovered field offset, or `None` if the routine does not
/// match the expected pattern.
pub fn find_structure_offset(function_address: u64) -> Option<u64> {
    let mut buffer = vec![0u8; PAGE_SIZE as usize];
    unsafe {
        ptr::copy_nonoverlapping(function_address as *const u8, buffer.as_mut_ptr(), buffer.len());
    }

    let mut decoder = Decoder::with_ip(64, &buffer, function_address, DecoderOptions::NONE);
    while decoder.can_decode() {
        let instruction = decoder.decode();
        if instruction.mnemonic() == Mnemonic::Ret {
            return None;
        }
        if instruction.mnemonic() == Mnemonic::Mov
            && instruction.op0_kind() == OpKind::Register
            && instruction.op0_register() == Register::RAX
            && instruction.op1_kind() == OpKind::Memory
            && instruction.memory_base() == Register::RCX
            && instruction.memory_index() == Register::None
            && instruction.memory_segment() == Register::None
        {
            return Some(instruction.memory_displacement64());
        }
    }
    None
}

/// Cached offset of the `Peb` member within the `EPROCESS` structure.
///
/// The offset is discovered the first time [`search_peb_offset`] is
/// called, then cached for the lifetime of the driver.
static PEB_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Returns the Process Environment Block (PEB) associated with an
/// `EPROCESS`.
///
/// The PEB is read directly from the process object using a dynamically
/// discovered field offset.
///
/// # Arguments
///
/// * `eprocess` - Pointer to the target `EPROCESS`.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(peb))` if the PEB pointer was successfully read.
/// - `Ok(None)` if the field offset could not be recovered.
/// - `Err(SearchError)` if an error occurred while locating the field
///   offset.
///
/// # Notes
///
/// The returned pointer is **not** validated. The caller is responsible
/// for ensuring that the target process remains valid before using it.
pub fn get_process_peb(eprocess: PEPROCESS) -> Result<Option<u64>, SearchError> {
    read_eprocess_field(eprocess, search_peb_offset)
}

/// Recovers the offset of the `Peb` member within the `EPROCESS`
/// structure.
///
/// Rather than relying on hardcoded offsets, this function analyses the
/// implementation of `PsGetProcessPeb` and extracts the displacement
/// used to access the `Peb` field.
///
/// The recovered offset is cached after the first successful lookup.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(offset))` if the field offset was successfully recovered.
/// - `Ok(None)` if the routine does not match the expected accessor
///   pattern.
/// - `Err(SearchError)` if the exported routine could not be resolved.
pub fn search_peb_offset() -> Result<Option<u64>, SearchError> {
    search_cached_offset(&PEB_OFFSET, r"\SystemRoot\System32\ntoskrnl.exe", "PsGetProcessPeb")
}


/// Cached offset of the `UniqueProcessId` member within the `EPROCESS`
/// structure.
///
/// The offset is discovered the first time
/// [`search_process_id_offset`] is called, then cached for the lifetime
/// of the driver.
static PROCESS_ID_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Returns the process identifier associated with an `EPROCESS`.
///
/// The process identifier is read directly from the process object using
/// a dynamically discovered field offset.
///
/// # Arguments
///
/// * `eprocess` - Pointer to the target `EPROCESS`.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(pid))` if the process identifier was successfully read.
/// - `Ok(None)` if the field offset could not be recovered.
/// - `Err(SearchError)` if an error occurred while locating the field
///   offset.
pub fn get_process_id(eprocess: PEPROCESS) -> Result<Option<u64>, SearchError> {
    read_eprocess_field(eprocess, search_process_id_offset)
}

/// Recovers the offset of the `UniqueProcessId` member within the
/// `EPROCESS` structure.
///
/// Rather than relying on hardcoded offsets, this function analyses the
/// implementation of `PsGetProcessId` and extracts the displacement
/// used to access the `UniqueProcessId` field.
///
/// The recovered offset is cached after the first successful lookup.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(offset))` if the field offset was successfully recovered.
/// - `Ok(None)` if the routine does not match the expected accessor
///   pattern.
/// - `Err(SearchError)` if the exported routine could not be resolved.
pub fn search_process_id_offset() -> Result<Option<u64>, SearchError> {
    search_cached_offset(&PROCESS_ID_OFFSET, r"\SystemRoot\System32\ntoskrnl.exe", "PsGetProcessId")
}


/// Cached offset of the `SectionBaseAddress` member within the
/// `EPROCESS` structure.
///
/// The offset is discovered the first time
/// [`search_process_image_base_offset`] is called, then cached for the
/// lifetime of the driver.
static PROCESS_IMAGE_BASE_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Returns the image base address of a process.
///
/// The image base corresponds to the address where the executable image
/// is mapped within the process address space.
///
/// # Arguments
///
/// * `eprocess` - Pointer to the target `EPROCESS`.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(image_base))` if the image base was successfully read.
/// - `Ok(None)` if the field offset could not be recovered.
/// - `Err(SearchError)` if an error occurred while locating the field
///   offset.
pub fn get_process_image_base(eprocess: PEPROCESS) -> Result<Option<u64>, SearchError> {
    read_eprocess_field(eprocess, search_process_image_base_offset)
}

/// Recovers the offset of the `SectionBaseAddress` member within the
/// `EPROCESS` structure.
///
/// Rather than relying on hardcoded offsets, this function analyses the
/// implementation of `PsGetProcessSectionBaseAddress` and extracts the
/// displacement used to access the `SectionBaseAddress` field.
///
/// The recovered offset is cached after the first successful lookup.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(Some(offset))` if the field offset was successfully recovered.
/// - `Ok(None)` if the routine does not match the expected accessor
///   pattern.
/// - `Err(SearchError)` if the exported routine could not be resolved.
pub fn search_process_image_base_offset() -> Result<Option<u64>, SearchError> {
    search_cached_offset(&PROCESS_IMAGE_BASE_OFFSET, r"\SystemRoot\System32\ntoskrnl.exe", "PsGetProcessSectionBaseAddress")
}



/// Initializes all known `EPROCESS` field offsets.
///
/// This function eagerly resolves and caches every supported field offset
/// instead of waiting for the first access. Calling this function during
/// driver initialization allows subsequent lookups to complete without
/// performing any routine resolution or disassembly.
///
/// Each supported offset discovery routine is executed exactly once. If
/// every offset is successfully recovered, they remain cached for the
/// lifetime of the driver.
///
/// # Returns
///
/// Returns:
///
/// - `Ok(true)` if every supported offset was successfully recovered.
/// - `Ok(false)` if at least one offset could not be determined.
/// - `Err(SearchError)` if an error occurred while resolving one of the
///   required kernel routines.
///
/// # Notes
///
/// This function is optional. Offset discovery is performed lazily by
/// each accessor if this function has not been called beforehand.
pub fn initialize_cached_offsets() -> Result<bool, SearchError> {
    let offset_search = [search_process_image_base_offset, search_process_id_offset, search_peb_offset];
    for search in offset_search {
        if search()?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}
