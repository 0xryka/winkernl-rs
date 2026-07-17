//! Low-level kernel memory allocation primitives.
//!
//! This module provides thin wrappers around the native Windows kernel
//! allocation APIs.
//!
//! Unlike the types exposed by the crate's `kalloc` module, the functions
//! defined here do **not** provide ownership or automatic resource
//! management. They simply expose the underlying allocation primitives and
//! are primarily intended to serve as building blocks for higher-level
//! abstractions.
//!
//! Most users should prefer using the RAII allocation types provided by
//! `crate::kalloc`, such as `PoolAllocator`, `AllocPoolSlice`,
//! `AllocContiguous`, or `AllocContiguousSlice`.
use core::ptr;
use crate::*;


/// Allocates a block of memory from a Windows kernel pool.
///
/// # Arguments
///
/// * `pool_type` - Pool from which the allocation should be performed.
/// * `size` - Allocation size in bytes.
///
/// # Returns
///
/// Returns the allocation address, or `None` if the allocation fails.
pub fn alloc_pool(pool_type: POOL_TYPE, size: usize) -> Option<PVOID> {
    unsafe {
        let mem = ExAllocatePool(pool_type, size as _);
        if mem.is_null() {
            return None;
        }
        Some(mem)
    }
}

/// Allocates enough pool memory to hold a value of type `T`.
///
/// This is a typed convenience wrapper around [`alloc_pool`].
pub fn alloc_pool_t<T>(pool_type: POOL_TYPE) -> Option<*mut T> {
    alloc_pool(pool_type, size_of::<T>()).map(|mem| mem as _)
}


/// Releases a pool allocation previously returned by [`alloc_pool`].
///
/// # Safety
///
/// The supplied pointer must have been allocated by the corresponding pool
/// allocation routines and must not have been freed previously.
pub fn free_pool(mem: *mut u8) {
    unsafe {
        ExFreePool(mem as _);
    }
}

/// Allocates physically contiguous memory.
///
/// The allocation is zero-initialized before being returned.
///
/// # Arguments
///
/// * `size` - Allocation size in bytes.
/// * `mem_caching_type` - Caching policy to use for the allocation.
///
/// # Returns
///
/// Returns the base address of the allocation, or [`None`] if the allocation
/// fails.
pub fn alloc_contiguous_memory(size: usize, mem_caching_type: MEMORY_CACHING_TYPE) -> Option<PVOID> {
    unsafe {
        let lowest = PHYSICAL_ADDRESS { QuadPart: 0 };
        let highest = PHYSICAL_ADDRESS { QuadPart: i64::MAX };
        let boundary = PHYSICAL_ADDRESS { QuadPart: 0 };

        let mem = MmAllocateContiguousMemorySpecifyCacheNode(size as _, lowest, highest, boundary, mem_caching_type, MM_ANY_NODE_OK);
        if mem.is_null() {
            return None;
        }
        ptr::write_bytes(mem as *mut u8, 0, size);
        Some(mem)
    }
}


/// Allocates enough physically contiguous memory to hold a value of type
/// `T`.
///
/// This is a typed convenience wrapper around
/// [`alloc_contiguous_memory`].
pub fn alloc_contiguous_memory_t<T>(mm_caching_t: MEMORY_CACHING_TYPE) -> Option<*mut T> {
    alloc_contiguous_memory(size_of::<T>(), mm_caching_t).map(|ptr| ptr as *mut T)
}


/// Releases a physically contiguous allocation.
///
/// # Arguments
///
/// * `base_addr` - Base address of the allocation.
/// * `size` - Allocation size in bytes.
/// * `mm_caching_type` - Caching policy used during allocation.
pub fn free_contiguous_memory(base_addr: PVOID, size: usize, mm_caching_type: MEMORY_CACHING_TYPE) {
    unsafe {
        MmFreeContiguousMemorySpecifyCache(base_addr, size as _, mm_caching_type);
    }
}