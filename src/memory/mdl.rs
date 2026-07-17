//! Memory Descriptor List (MDL) utilities.
//!
//! This module provides safe RAII wrappers around Windows kernel
//! **MDLs**.
//!
//! MDLs describe a virtual memory region by the physical pages backing it
//! and allow the kernel to lock, map and modify memory through the Memory
//! Manager.
//!
//! These wrappers automatically release every acquired resource, avoiding
//! common mistakes such as forgetting to unlock pages or unmap a mapped
//! region.
//!
//! # Overview
//!
//! - [`Mdl`] owns a Windows MDL and manages its lifetime.
//! - [`MdlMap`] represents a temporary system mapping created from a locked
//!   MDL.
//!
//! # Typical usage
//!
//! 1. Create an [`Mdl`] for a virtual memory region.
//! 2. Lock the pages with [`Mdl::lock`].
//! 3. Map the pages into system address space with [`Mdl::map_locked`].
//! 4. Optionally change the mapping protection using
//!    [`MdlMap::protect`].
//! 5. Access the mapped memory.
//!
//! When the objects leave scope:
//!
//! - [`MdlMap`] automatically calls `MmUnmapLockedPages`.
//! - [`Mdl`] automatically calls `MmUnlockPages` (if necessary).
//! - [`Mdl`] automatically frees the underlying MDL with `IoFreeMdl`.
//!
//! # Safety
//!
//! This module only manages the lifetime of MDLs.
//!
//! The caller remains responsible for ensuring that the supplied virtual
//! address range is valid. In particular, `MmProbeAndLockPages` may raise
//! a structured exception if an invalid address is supplied.
use core::ops::{Deref, DerefMut};
use core::ptr;
use x86_64::VirtAddr;
use crate::sys::*;


/// Owns a Windows Memory Descriptor List (MDL).
///
/// An `Mdl` is responsible for locking a virtual memory region,
/// creating temporary system mappings and releasing every acquired
/// kernel resource when dropped.
pub struct Mdl {
    mdl: PMDL,
    locked: bool,
}



/// Represents a temporary mapping created from a locked [`Mdl`].
///
/// Dropping this object automatically unmaps the system mapping by
/// calling `MmUnmapLockedPages`.
pub struct MdlMap<'a, T> {
    mdl_ref: &'a Mdl,
    mapped: *mut T,
}


impl<T> Drop for MdlMap<'_, T> {
    fn drop(&mut self) {
        unsafe {
            MmUnmapLockedPages(self.mapped as _, self.mdl_ref.mdl);
        }
    }
}



impl<T> MdlMap<'_, T> {
    pub fn as_ptr(&self) -> *const T {
        self.mapped
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.mapped
    }

    /// Changes the protection of the mapped pages.
    ///
    /// This is a thin wrapper around `MmProtectMdlSystemAddress`.
    pub fn protect(&self, new_protect: u32) {
        self.mdl_ref.protect(new_protect);
    }
}


impl Mdl {
    /// Creates an MDL describing the given virtual memory range.
    ///
    /// The pages are **not** locked automatically. Call [`Mdl::lock`]
    /// before attempting to map the memory.
    pub fn new(dest: VirtAddr, size: usize) -> Option<Self> {
        unsafe {
            let mdl = IoAllocateMdl(dest.as_u64() as _, size as _, false as _, false as _, ptr::null_mut());
            if mdl.is_null() {
                return None;
            }
            Some(Self {
                mdl, locked: false
            })
        }
    }

    /// Locks the pages described by this MDL.
    ///
    /// # Safety
    ///
    /// This function ultimately calls `MmProbeAndLockPages`, which may
    /// raise a structured exception if the supplied address range is
    /// invalid.
    pub fn lock(&mut self, access_mode: KPROCESSOR_MODE, lock_operation: LOCK_OPERATION) {
        unsafe {
            MmProbeAndLockPages(self.mdl, access_mode, lock_operation);
        }
        self.locked = true;
    }



    pub fn unlock(&mut self) {
        unsafe {
            MmUnlockPages(self.mdl);
        }
        self.locked = false;
    }


    /// Maps the locked pages into the system address space.
    ///
    /// Returns an [`MdlMap`] that automatically unmaps the pages when
    /// dropped.
    ///
    /// The pages must have been locked beforehand.
    pub fn map_locked<'a, T>(&'a mut self, access_mode: KPROCESSOR_MODE, mm_caching: MEMORY_CACHING_TYPE, request_addr: Option<VirtAddr>, bug_check_on_failure: bool, mm_page_priority: MM_PAGE_PRIORITY) -> Option<MdlMap<'a, T>> {
        unsafe {
            let mapping = MmMapLockedPagesSpecifyCache(self.mdl, access_mode, mm_caching, request_addr.map(|m|m.as_u64() as _).unwrap_or(ptr::null_mut()), bug_check_on_failure as _, mm_page_priority as _);
            if mapping.is_null() {
                return None;
            }
            Some(MdlMap {
                mdl_ref: self,
                mapped: mapping as _,
            })
        }
    }

    /// Changes the protection of the mapped pages.
    ///
    /// This is a thin wrapper around `MmProtectMdlSystemAddress`.
    pub fn protect(&self, new_protect: u32) {
        unsafe {
            MmProtectMdlSystemAddress(self.mdl, new_protect);
        }
    }
}


impl Drop for Mdl {
    fn drop(&mut self) {
        if self.locked {
            self.unlock();
        }
        unsafe {
            IoFreeMdl(self.mdl);
        }
    }
}


