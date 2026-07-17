//! Kernel memory allocation utilities.
//!
//! This module provides allocation primitives backed by the Windows kernel
//! memory manager.
//!
//! It contains:
//!
//! - [`KernelAllocator`], a global allocator implementation using
//!   `ExAllocatePool`/`ExFreePool`.
//! - [`pool`], safe abstractions over Windows kernel pool allocations.
//! - [`contiguous`], abstractions for physically contiguous memory
//!   allocations intended for DMA and other hardware-related operations.
//!
//! ## Global allocator
//!
//! The [`KernelAllocator`] type implements [`core::alloc::GlobalAlloc`] and
//! can be installed as the crate's global allocator:
//!
//! ```rust,no_run
//! use winkernl_rs::kalloc::KernelAllocator;
//!
//! #[global_allocator]
//! static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;
//! ```
//!
//! Once installed, standard allocation types such as `Box`, `Vec`, `String`,
//! and collections from the `alloc` crate will allocate memory directly from
//! the Windows kernel pool.
//!
//! ## Notes
//!
//! The global allocator always allocates from the **NonPagedPool**.
//!
//! If a different allocation strategy is required (for example paged memory,
//! executable pool, or physically contiguous memory), prefer the allocation
//! wrappers provided by the [`pool`] and [`contiguous`] modules instead.
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use crate::sys::{ExAllocatePool, ExFreePool, POOL_TYPE};
pub mod contiguous;
pub mod pool;

/// Global allocator backed by the Windows kernel pool.
///
/// Every allocation is performed through `ExAllocatePool` using the
/// `NonPagedPool` pool type, ensuring that allocated memory always remains
/// resident in physical memory.
///
/// This allocator is intended to be used as the crate's global allocator
/// in kernel-mode environments.
///
/// # Notes
///
/// Memory allocated through this allocator is released with `ExFreePool`.
///
/// Since this allocator always uses `NonPagedPool`, it is not suitable for
/// allocations requiring a different pool type (such as executable or
/// paged memory).
pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    /// Allocates a block of memory from the Windows kernel pool.
    ///
    /// The requested allocation size is taken from `layout.size()`. The
    /// alignment specified by the layout is ignored, as alignment guarantees
    /// are provided by the underlying kernel allocator.
    ///
    /// # Returns
    ///
    /// Returns a valid pointer on success, or a null pointer if the
    /// allocation fails.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let mem = unsafe {
            ExAllocatePool(POOL_TYPE::NonPagedPool, size as _)
        };

        if mem.is_null() {
            ptr::null_mut()
        } else {
            mem.cast()
        }
    }

    /// Releases a block previously allocated by [`KernelAllocator`].
    ///
    /// If `ptr` is null, this function performs no operation.
    ///
    /// The `layout` parameter is ignored since `ExFreePool` only requires
    /// the allocation address.
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if !ptr.is_null() {
            unsafe {
                ExFreePool(ptr as _);
            }
        }
    }
}