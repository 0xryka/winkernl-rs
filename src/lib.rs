//! # WinKernel-rs
//!
//! `WinKernel-rs` is a Rust library designed to simplify Windows kernel-mode
//! (Ring 0) and low-level systems development.
//!
//! Rather than exposing only thin wrappers around Windows kernel APIs, this
//! crate provides safe(er), idiomatic Rust abstractions for many common kernel
//! programming tasks while still allowing direct access to low-level
//! primitives when required.
//!
//! The goal of the crate is to let developers focus on implementing kernel
//! logic instead of repeatedly writing boilerplate around the Windows Driver
//! Kit (WDK).
//!
//! ## Features
//!
//! The library provides abstractions for several aspects of kernel-mode
//! development, including:
//!
//! - Windows kernel memory allocation.
//! - Physically contiguous memory.
//! - Kernel pool allocation.
//! - Virtual and physical memory manipulation.
//! - Page table walking and address translation.
//! - Safe wrappers around common WDK routines.
//! - Processor and CPU utilities.
//! - Synchronization primitives.
//! - Hypervisor and virtualization helpers.
//! - Miscellaneous low-level utilities.
//!
//! ## Memory abstractions
//!
//! Several RAII allocation wrappers are provided.
//!
//! These wrappers automatically release their underlying allocations and
//! expose ergonomic Rust APIs similar to `Box<T>` or `Box<[T]>`.
//!
//! Examples include:
//!
//! - `PoolMemory<T>`
//! - `AllocPoolSlice<T>`
//! - `ContiguousMemory<T>`
//! - `ContiguousMemorySlice<T>`
//!
//! They also provide support for:
//!
//! - zeroed allocations;
//! - uninitialized allocations through `MaybeUninit`;
//! - in-place initialization;
//! - intentional leaking when ownership must be transferred.
//!
//! ## Low-level memory utilities
//!
//! The crate contains helpers for interacting with Windows virtual memory,
//! physical memory and paging structures.
//!
//! These include:
//!
//! - software page table walking;
//! - virtual ↔ physical address translation;
//! - physical memory access;
//! - page table manipulation;
//! - CR3 utilities.
//!
//! ## Hypervisor support
//!
//! The library contains utilities intended for virtualization-based projects,
//! including helpers for:
//!
//! - AMD-V (SVM);
//! - Nested Page Tables (NPT);
//! - VMEXIT handling;
//! - MSR permission maps;
//! - virtualization-related processor state.
//!
//! ## Design goals
//!
//! This crate follows several principles:
//!
//! - provide ergonomic Rust abstractions;
//! - remain close to native Windows kernel APIs;
//! - avoid unnecessary runtime overhead;
//! - leverage RAII whenever ownership exists;
//! - expose low-level primitives instead of hiding them.
//!
//! Whenever zero-cost abstractions are possible, they are preferred.
//!
//! ## Safety
//!
//! Although the crate provides higher-level abstractions, it targets kernel
//! development where many operations are inherently unsafe.
//!
//! APIs that cannot guarantee memory safety are therefore explicitly marked
//! `unsafe`, leaving responsibility to the caller when required.
//!
//! ## Scope
//!
//! This crate is intended for:
//!
//! - Windows kernel drivers;
//! - Hypervisors;
//! - Research projects;
//! - Operating-system development;
//! - Low-level tooling;
//! - Security software;
//! - Reverse engineering tools.
//!
//! It is **not** intended to completely hide the Windows kernel APIs.
//! Instead, it provides reusable building blocks that make kernel development
//! significantly more ergonomic while preserving full access to the underlying
//! platform.
#![no_std]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(unused_unsafe)]
#![allow(unused_parens)]
#![allow(clippy::all)]
#![allow(improper_ctypes)]

extern crate alloc;
pub extern crate x86_64;
use alloc::vec::Vec;
use core::{ptr, slice};
use core::ffi::c_void;

pub mod memory;
pub mod khook;
pub mod kalloc;
pub mod sys;
use sys::*;
pub mod process;
use sys::ntstatus::*;




#[inline(always)]
pub const fn nt_success(status: NTSTATUS) -> bool {
    status >= 0
}
#[repr(C)]
pub struct DBGKD_DEBUG_DATA_HEADER64 {
    pub List: LIST_ENTRY64,
    pub OwnerTag: u32,
    pub Size: u32,
}

#[repr(C)]
pub struct KDDEBUGGER_DATA64 {
    pub Header: DBGKD_DEBUG_DATA_HEADER64,
    pub KernBase: u64,
    pub BreakpointWithStatus: u64,
    pub SavedContext: u64,
    pub ThCallbackStack: u16,
    pub NextCallback: u16,
    pub FramePointer: u16,
    pub _bitfield: u16,
    pub KiCallUserMode: u64,
    pub KeUserCallbackDispatcher: u64,
    pub PsLoadedModuleList: u64,
    pub PsActiveProcessHead: u64,
    pub PspCidTable: u64,
    pub ExpSystemResourcesList: u64,
    pub ExpPagedPoolDescriptor: u64,
    pub ExpNumberOfPagedPools: u64,
    pub KeTimeIncrement: u64,
    pub KeBugCheckCallbackListHead: u64,
    pub KiBugcheckData: u64,
    pub IopErrorLogListHead: u64,
    pub ObpRootDirectoryObject: u64,
    pub ObpTypeObjectType: u64,
    pub MmSystemCacheStart: u64,
    pub MmSystemCacheEnd: u64,
    pub MmSystemCacheWs: u64,
    pub MmPfnDatabase: u64,
    pub MmSystemPtesStart: u64,
    pub MmSystemPtesEnd: u64,
    pub MmSubsectionBase: u64,
    pub MmNumberOfPagingFiles: u64,
    pub MmLowestPhysicalPage: u64,
    pub MmHighestPhysicalPage: u64,
    pub MmNumberOfPhysicalPages: u64,
    pub MmMaximumNonPagedPoolInBytes: u64,
    pub MmNonPagedSystemStart: u64,
    pub MmNonPagedPoolStart: u64,
    pub MmNonPagedPoolEnd: u64,
    pub MmPagedPoolStart: u64,
    pub MmPagedPoolEnd: u64,
    pub MmPagedPoolInformation: u64,
    pub MmPageSize: u64,
    pub MmSizeOfPagedPoolInBytes: u64,
    pub MmTotalCommitLimit: u64,
    pub MmTotalCommittedPages: u64,
    pub MmSharedCommit: u64,
    pub MmDriverCommit: u64,
    pub MmProcessCommit: u64,
    pub MmPagedPoolCommit: u64,
    pub MmExtendedCommit: u64,
    pub MmZeroedPageListHead: u64,
    pub MmFreePageListHead: u64,
    pub MmStandbyPageListHead: u64,
    pub MmModifiedPageListHead: u64,
    pub MmModifiedNoWritePageListHead: u64,
    pub MmAvailablePages: u64,
    pub MmResidentAvailablePages: u64,
    pub PoolTrackTable: u64,
    pub NonPagedPoolDescriptor: u64,
    pub MmHighestUserAddress: u64,
    pub MmSystemRangeStart: u64,
    pub MmUserProbeAddress: u64,
    pub KdPrintCircularBuffer: u64,
    pub KdPrintCircularBufferEnd: u64,
    pub KdPrintWritePointer: u64,
    pub KdPrintRolloverCount: u64,
    pub MmLoadedUserImageList: u64,
    pub NtBuildLab: u64,
    pub KiNormalSystemCall: u64,
    pub KiProcessorBlock: u64,
    pub MmUnloadedDrivers: u64,
    pub MmLastUnloadedDriver: u64,
    pub MmTriageActionTaken: u64,
    pub MmSpecialPoolTag: u64,
    pub KernelVerifier: u64,
    pub MmVerifierData: u64,
    pub MmAllocatedNonPagedPool: u64,
    pub MmPeakCommitment: u64,
    pub MmTotalCommitLimitMaximum: u64,
    pub CmNtCSDVersion: u64,
    pub MmPhysicalMemoryBlock: u64,
    pub MmSessionBase: u64,
    pub MmSessionSize: u64,
    pub MmSystemParentTablePage: u64,
    pub MmVirtualTranslationBase: u64,
    pub OffsetKThreadNextProcessor: u16,
    pub OffsetKThreadTeb: u16,
    pub OffsetKThreadKernelStack: u16,
    pub OffsetKThreadInitialStack: u16,
    pub OffsetKThreadApcProcess: u16,
    pub OffsetKThreadState: u16,
    pub OffsetKThreadBStore: u16,
    pub OffsetKThreadBStoreLimit: u16,
    pub SizeEProcess: u16,
    pub OffsetEprocessPeb: u16,
    pub OffsetEprocessParentCID: u16,
    pub OffsetEprocessDirectoryTableBase: u16,
    pub SizePrcb: u16,
    pub OffsetPrcbDpcRoutine: u16,
    pub OffsetPrcbCurrentThread: u16,
    pub OffsetPrcbMhz: u16,
    pub OffsetPrcbCpuType: u16,
    pub OffsetPrcbVendorString: u16,
    pub OffsetPrcbProcStateContext: u16,
    pub OffsetPrcbNumber: u16,
    pub SizeEThread: u16,
    pub L1tfHighPhysicalBitIndex: u8,
    pub L1tfSwizzleBitIndex: u8,
    pub Padding0: u32,
    pub KdPrintCircularBufferPtr: u64,
    pub KdPrintBufferSize: u64,
    pub KeLoaderBlock: u64,
    pub SizePcr: u16,
    pub OffsetPcrSelfPcr: u16,
    pub OffsetPcrCurrentPrcb: u16,
    pub OffsetPcrContainedPrcb: u16,
    pub OffsetPcrInitialBStore: u16,
    pub OffsetPcrBStoreLimit: u16,
    pub OffsetPcrInitialStack: u16,
    pub OffsetPcrStackLimit: u16,
    pub OffsetPrcbPcrPage: u16,
    pub OffsetPrcbProcStateSpecialReg: u16,
    pub GdtR0Code: u16,
    pub GdtR0Data: u16,
    pub GdtR0Pcr: u16,
    pub GdtR3Code: u16,
    pub GdtR3Data: u16,
    pub GdtR3Teb: u16,
    pub GdtLdt: u16,
    pub GdtTss: u16,
    pub Gdt64R3CmCode: u16,
    pub Gdt64R3CmTeb: u16,
    pub IopNumTriageDumpDataBlocks: u64,
    pub IopTriageDumpDataBlocks: u64,
    pub VfCrashDataBlock: u64,
    pub MmBadPagesDetected: u64,
    pub MmZeroedPageSingleBitErrorsDetected: u64,
    pub EtwpDebuggerData: u64,
    pub OffsetPrcbContext: u16,
    pub OffsetPrcbMaxBreakpoints: u16,
    pub OffsetPrcbMaxWatchpoints: u16,
    pub OffsetKThreadStackLimit: u32,
    pub OffsetKThreadStackBase: u32,
    pub OffsetKThreadQueueListEntry: u32,
    pub OffsetEThreadIrpList: u32,
    pub OffsetPrcbIdleThread: u16,
    pub OffsetPrcbNormalDpcState: u16,
    pub OffsetPrcbDpcStack: u16,
    pub OffsetPrcbIsrStack: u16,
    pub SizeKDPC_STACK_FRAME: u16,
    pub OffsetKPriQueueThreadListHead: u16,
    pub OffsetKThreadWaitReason: u16,
    pub Padding1: u16,
    pub PteBase: u64,
    pub RetpolineStubFunctionTable: u64,
    pub RetpolineStubFunctionTableSize: u32,
    pub RetpolineStubOffset: u32,
    pub RetpolineStubSize: u32,
    pub OffsetEProcessMmHotPatchContext: u16,
    pub OffsetKThreadShadowStackLimit: u32,
    pub OffsetKThreadShadowStackBase: u32,
    pub ShadowStackEnabled: u64,
    pub PointerAuthMask: u64,
    pub OffsetPrcbExceptionStack: u16,
}


#[repr(C)]
pub struct IMAGE_DOS_HEADER {
    pub e_magic: u16,
    pub e_cblp: u16,
    pub e_cp: u16,
    pub e_crlc: u16,
    pub e_cparhdr: u16,
    pub e_minalloc: u16,
    pub e_maxalloc: u16,
    pub e_ss: u16,
    pub e_sp: u16,
    pub e_csum: u16,
    pub e_ip: u16,
    pub e_cs: u16,
    pub e_lfarlc: u16,
    pub e_ovno: u16,
    pub e_res: [u16; 4],
    pub e_oemid: u16,
    pub e_oeminfo: u16,
    pub e_res2: [u16; 10],
    pub e_lfanew: i32,
}


#[repr(C)]
pub struct IMAGE_NT_HEADERS64 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}



#[repr(C)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: u16,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: u16,
}


#[repr(C)]
pub struct IMAGE_DATA_DIRECTORY {
    pub VirtualAddress: u32,
    pub Size: u32,
}

#[repr(C)]
pub struct IMAGE_OPTIONAL_HEADER64 {
    pub Magic: u16,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub ImageBase: u64,
    pub SectionAlignment: u32,
    pub FileAlignment: u32,
    pub MajorOperatingSystemVersion: u16,
    pub MinorOperatingSystemVersion: u16,
    pub MajorImageVersion: u16,
    pub MinorImageVersion: u16,
    pub MajorSubsystemVersion: u16,
    pub MinorSubsystemVersion: u16,
    pub Win32VersionValue: u32,
    pub SizeOfImage: u32,
    pub SizeOfHeaders: u32,
    pub CheckSum: u32,
    pub Subsystem: u16,
    pub DllCharacteristics: u16,
    pub SizeOfStackReserve: u64,
    pub SizeOfStackCommit: u64,
    pub SizeOfHeapReserve: u64,
    pub SizeOfHeapCommit: u64,
    pub LoaderFlags: u32,
    pub NumberOfRvaAndSizes: u32,
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}


#[repr(C)]
#[derive(Copy, Clone)]
pub struct RTL_PROCESS_MODULE_INFORMATION {
    pub Section: PVOID,
    pub MappedBase: PVOID,
    pub ImageBase: PVOID,
    pub ImageSize: ULONG,
    pub Flags: ULONG,
    pub LoadOrderIndex: USHORT,
    pub InitOrderIndex: USHORT,
    pub LoadCount: USHORT,
    pub OffsetToFileName: USHORT,
    pub FullPathName: [UCHAR; 256],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct RTL_PROCESS_MODULES {
    pub NumberOfModules: ULONG,
    pub Modules: [RTL_PROCESS_MODULE_INFORMATION; 1],
}


pub const SystemModuleInformation: u32 = 11;
pub const SystemProcessInformation: u32 = 5;
pub const IoReadAccess: i32 = 1;
pub const MmNonCached: i32 = 0;


#[repr(C)]
pub struct SYSTEM_PROCESS_INFORMATION {
    pub NextEntryOffset: ULONG,
    pub NumberOfThreads: ULONG,
    pub Reserved1: [u64; 3],
    pub CreateTime: i64,
    pub UserTime: i64,
    pub KernelTime: i64,
    pub ImageName: UNICODE_STRING,
    pub BasePriority: i32,
    pub UniqueProcessId: PVOID,
    pub InheritedFromUniqueProcessId: PVOID,
}



#[repr(C)]
#[derive(Copy, Clone)]
pub union MouseButtons {
    pub Buttons: ULONG,
    pub ButtonFields: ButtonFields,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ButtonFields {
    pub ButtonFlags: USHORT,
    pub ButtonData: USHORT,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MOUSE_INPUT_DATA {
    pub UnitId: USHORT,
    pub Flags: USHORT,
    pub u: MouseButtons,
    pub RawButtons: ULONG,
    pub LastX: LONG,
    pub LastY: LONG,
    pub ExtraInformation: ULONG,
}


pub type PMOUSE_INPUT_DATA = *mut MOUSE_INPUT_DATA;

pub type MouseClassServiceCallbackFn = Option<unsafe extern "system" fn(DeviceObject: PDEVICE_OBJECT, InputDataStart: PMOUSE_INPUT_DATA, InputDataEnd: PMOUSE_INPUT_DATA, InputDataConsumed: PULONG)>;



#[derive(Copy, Clone)]
pub struct MOUSE_OBJECT {
    pub mouse_device: PDEVICE_OBJECT,
    pub service_call_back: MouseClassServiceCallbackFn,
    pub use_mouse: i32,
}




unsafe extern "C" {
    pub static IoDriverObjectType: *mut *mut u8;
    pub fn PsLookupProcessByProcessId(ProcessId: HANDLE, Process: *mut PEPROCESS) -> NTSTATUS;
    pub fn ZwQuerySystemInformation(SystemInformationClass: u32, SystemInformation: PVOID, SystemInformationLength: ULONG, ReturnLength: *mut ULONG) -> NTSTATUS;
    pub fn RtlSecureZeroMemory(Destination: PVOID, Length: SIZE_T) -> PVOID;
    pub fn RtlFindExportedRoutineByName(image_base: PVOID, routine_name: *const u8) -> PVOID;
    pub fn PsGetProcessPeb(pep: PEPROCESS) -> u64;
    pub fn MmCopyVirtualMemory(FromProcess: PEPROCESS, FromAddress: PVOID, ToProcess: PEPROCESS, ToAddress: PVOID, BufferSize: SIZE_T,
                               PreviousMode: KPROCESSOR_MODE, NumberOfBytesCopied: *mut SIZE_T) -> NTSTATUS;

    pub fn PsGetCurrentProcess() -> PEPROCESS;
    pub fn PsGetProcessSectionBaseAddress(Process: PEPROCESS) -> PVOID;
    pub fn ObReferenceObjectByName(ObjectName: *mut UNICODE_STRING, Attributes: ULONG, PassedAccessState: *mut ACCESS_STATE, DesiredAccess: ACCESS_MASK, ObjectType: *mut u8,
    AccessMode: u8, ParseContext: *mut c_void, Object: *mut *mut c_void) -> NTSTATUS;
    pub fn ZwCurrentProcess() -> HANDLE;
    pub fn IoCreateDriver(DriverName: *mut UNICODE_STRING, InitializationFunction: extern "system" fn(*mut c_void, *mut UNICODE_STRING) -> NTSTATUS) -> NTSTATUS;
}




pub fn modules_from_ptr<'a>(ptr: *const RTL_PROCESS_MODULES) -> &'a [RTL_PROCESS_MODULE_INFORMATION] {
    unsafe {
        if ptr.is_null() {
            &[]
        } else {
            let count = (*ptr).NumberOfModules as usize;
            let first = &(*ptr).Modules as *const RTL_PROCESS_MODULE_INFORMATION;
            slice::from_raw_parts(first, count)
        }
    }
}


pub fn filename_from_info(info: &RTL_PROCESS_MODULE_INFORMATION) -> &[u8] {
    let off = info.OffsetToFileName as usize;
    if off >= info.FullPathName.len() {
        if let Some(pos) = info.FullPathName.iter().position(|&b| b == 0) {
            &info.FullPathName[..pos]
        } else {
            &info.FullPathName[..]
        }
    } else {
        let slice = &info.FullPathName[off..];
        if let Some(pos) = slice.iter().position(|&b| b == 0) {
            &slice[..pos]
        } else {
            slice
        }
    }
}

pub fn module_base_and_name(info: &RTL_PROCESS_MODULE_INFORMATION) -> (PVOID, &[u8]) {
    (info.ImageBase, filename_from_info(info))
}


pub fn init_object_attributes(obj: &mut OBJECT_ATTRIBUTES, name: *mut UNICODE_STRING, attributes: u32) {
    obj.Length = size_of::<OBJECT_ATTRIBUTES>() as u32;
    obj.RootDirectory = ptr::null_mut();
    obj.ObjectName = name;
    obj.Attributes = attributes;
    obj.SecurityDescriptor = ptr::null_mut();
    obj.SecurityQualityOfService = ptr::null_mut();
}
