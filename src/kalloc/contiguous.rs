//! Physically contiguous memory allocation.
//!
//! This module provides safe ownership wrappers around physically contiguous
//! memory allocated from the Windows kernel.
//!
//! Unlike pool allocations, contiguous allocations are backed by physically
//! adjacent pages. This makes them suitable for scenarios where the physical
//! address of the allocation matters, such as:
//!
//! - Direct Memory Access (DMA).
//! - Hardware communication.
//! - Page table construction or manipulation.
//! - Nested paging (EPT/NPT).
//! - Hypervisors and virtualization.
//!
//! All allocations are automatically released when their owning object is
//! dropped.
//!
//! ## Types
//!
//! Two allocation wrappers are provided:
//!
//! - [`ContiguousMemory<T>`] owns a single contiguous object.
//! - [`ContiguousMemorySlice<T>`] owns a contiguous array of elements whose
//!   length is determined at runtime.
//!
//! Both wrappers:
//!
//! - allocate contiguous physical memory;
//! - automatically free the allocation on drop;
//! - implement `Deref` and `DerefMut`;
//! - provide constructors for initialized, zeroed and uninitialized memory;
//! - expose `MaybeUninit` constructors for manual initialization.
//!
//! ## Example
//!
//! Allocate and initialize a single object:
//!
//! ```rust,no_run
//! use winkernl_rs::kalloc::contiguous::ContiguousMemory;
//!
//! let value = ContiguousMemory::new(42u64).unwrap();
//! assert_eq!(*value, 42);
//! ```
//!
//! Allocate a runtime-sized contiguous buffer:
//!
//! ```rust,no_run
//! use winkernl_rs::kalloc::contiguous::ContiguousMemorySlice;
//!
//! let buffer = ContiguousMemorySlice::<u8>::new_zeroed(4096).unwrap();
//! assert_eq!(buffer.len(), 4096);
//! ```
//!
//! ## Initialization
//!
//! Constructors returning `MaybeUninit<T>` allow callers to initialize memory
//! manually before converting it into its initialized form with
//! `assume_init()`.
//!
//! Calling `assume_init()` is `unsafe` because the caller must guarantee that
//! every byte of the object (or every element of the slice) has been fully
//! initialized.
//!
//! ## Caching policy
//!
//! Every allocation stores the selected
//! [`MEMORY_CACHING_TYPE`](crate::sys::MEMORY_CACHING_TYPE).
//!
//! The caching policy is passed to the Windows memory manager when allocating
//! and freeing the contiguous region.
use core::{mem, ptr, slice};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use crate::memory::allocator::{alloc_contiguous_memory, alloc_contiguous_memory_t, free_contiguous_memory};
use crate::sys::MEMORY_CACHING_TYPE;

/// Owns a single physically contiguous kernel allocation.
///
/// The allocation contains exactly one value of type `T`.
///
/// Memory is automatically released when this object is dropped.
///
/// This type behaves similarly to `Box<T>`, except that the underlying
/// allocation is physically contiguous instead of coming from the regular
/// kernel pool.
///
/// Most users should use [`new`](Self::new) or
/// [`new_with_initializer`](Self::new_with_initializer). Manual
/// initialization through [`MaybeUninit`](MaybeUninit) is available for advanced use cases.
pub struct ContiguousMemory<T> {
    mem: ptr::NonNull<T>,
    mm_caching: MEMORY_CACHING_TYPE,
}




const DEFAULT_CACHING: MEMORY_CACHING_TYPE = MEMORY_CACHING_TYPE::MmCached;

impl<T> ContiguousMemory<T> {
    /// Allocates an uninitialized contiguous object.
    ///
    /// The allocation is zero-filled before being returned as
    /// `MaybeUninit<T>`.
    ///
    /// The returned allocation must be fully initialized before calling
    /// [`assume_init`](ContiguousMemory::<MaybeUninit<T>>::assume_init).
    #[inline(always)]
    pub fn new_uninit_zeroed() -> Option<ContiguousMemory<MaybeUninit<T>>> {
        Self::new_uninit_with_caching_type(DEFAULT_CACHING)
    }

    pub fn new_uninit_zeroed_with_caching_type(mm_caching: MEMORY_CACHING_TYPE) -> Option<ContiguousMemory<MaybeUninit<T>>> {
        let mem = alloc_contiguous_memory_t::<MaybeUninit<T>>(mm_caching)?;
        let mem = ptr::NonNull::new(mem)?;
        unsafe {
            mem.write_bytes(0, 1);
        }
        Some(ContiguousMemory { mem, mm_caching })
    }

    #[inline(always)]
    pub fn new_uninit() -> Option<ContiguousMemory<MaybeUninit<T>>> {
        Self::new_uninit_with_caching_type(DEFAULT_CACHING)
    }
    #[inline(always)]
    pub fn new_uninit_with_caching_type(mm_chaching: MEMORY_CACHING_TYPE) -> Option<ContiguousMemory<MaybeUninit<T>>> {
        Self::new_uninit_zeroed_with_caching_type(mm_chaching)
    }
}


impl<T> ContiguousMemory<MaybeUninit<T>> {
    /// Converts an initialized `MaybeUninit` allocation into its initialized form.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the entire allocation has been fully
    /// initialized.
    ///
    /// Calling this function on uninitialized memory results in undefined
    /// behavior.
    pub unsafe fn assume_init(self) -> ContiguousMemory<T> {
        let mem = self.mem.cast();
        let mm_caching = self.mm_caching;
        mem::forget(self);
        ContiguousMemory {
            mem,
            mm_caching,
        }
    }
}

impl<T> ContiguousMemory<T> {
    /// Allocates a contiguous object initialized with `value`.
    pub fn new(val: T) -> Option<ContiguousMemory<T>> {
        Self::new_with_caching_type(val, DEFAULT_CACHING)
    }

    pub fn new_with_caching_type(val: T, cache_type: MEMORY_CACHING_TYPE) -> Option<ContiguousMemory<T>> {
        let mem_ptr = alloc_contiguous_memory(size_of::<T>(), cache_type)?;
        let mem = ptr::NonNull::new(mem_ptr as *mut T)?;
        unsafe {
            mem.write(val);
        }
        Some(Self {
            mem,
            mm_caching: cache_type,
        })
    }


    /// Allocates a contiguous object and initializes it through a closure.
    ///
    /// This constructor avoids creating a temporary value and is especially
    /// useful for initializing self-referential or partially initialized
    /// structures.
    pub fn new_with_initializer<F: FnOnce(&mut MaybeUninit<T>)>(initializer: F) -> Option<ContiguousMemory<T>> {
        Self::new_with_initializer_and_caching_type(DEFAULT_CACHING, initializer)
    }

    pub fn new_with_initializer_and_caching_type<F: FnOnce(&mut MaybeUninit<T>)>(mm_caching: MEMORY_CACHING_TYPE, initializer: F) -> Option<ContiguousMemory<T>> {
        let mut self_uninit = Self::new_uninit_with_caching_type(mm_caching)?;
        initializer(&mut *self_uninit);
        unsafe { Some(self_uninit.assume_init()) }
    }

    pub unsafe fn new_zeroed() -> Option<ContiguousMemory<T>> {
        unsafe {
            Self::new_zeroed_with_caching_type(DEFAULT_CACHING)
        }
    }

    pub unsafe fn new_zeroed_with_caching_type(mm_caching: MEMORY_CACHING_TYPE) -> Option<ContiguousMemory<T>> {
        let self_uninit = Self::new_uninit_zeroed_with_caching_type(mm_caching)?;
        unsafe {
            Some(self_uninit.assume_init())
        }
    }


    /// Intentionally leaks the allocation.
    ///
    /// Ownership is relinquished and the allocation will never be freed
    /// automatically.
    ///
    /// The returned reference remains valid until the memory is explicitly
    /// released through another mechanism.
    pub fn leak(self) -> &'static mut T {
        unsafe {
            let ptr = self.mem.as_ptr();
            mem::forget(self);
            &mut *ptr
        }
    }


    pub fn as_slice<'a>(&'a self) -> &'a [T] {
        unsafe {
            slice::from_raw_parts(self.mem.as_ptr(), 1)
        }
    }

    pub fn as_mut_slice<'a>(&'a mut self) -> &'a mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.mem.as_ptr(), 1)
        }
    }

    pub unsafe fn as_slice_count<'a>(&'a self, count: usize) -> &'a [T] {
        unsafe {
            slice::from_raw_parts(self.mem.as_ptr(), count)
        }
    }

    pub unsafe fn as_mut_slice_count<'a>(&'a mut self, count: usize) -> &'a mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.mem.as_ptr(), count)
        }
    }
    pub fn as_ptr(&self) -> *const T {
        self.mem.as_ptr() as _
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.mem.as_ptr()
    }
}



impl<T> Drop for ContiguousMemory<T> {
    fn drop(&mut self) {
        unsafe {
            self.mem.as_ptr().drop_in_place();
        }
        free_contiguous_memory(self.mem.as_ptr() as _, size_of::<T>(), self.mm_caching);
    }
}


impl<T> Deref for ContiguousMemory<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.mem.as_ref() }
    }
}

impl<T> DerefMut for ContiguousMemory<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.mem.as_mut() }
    }
}



/// Owns a dynamically sized contiguous allocation.
///
/// This type is the contiguous-memory equivalent of `Box<[T]>`.
///
/// The allocation contains `len` consecutive values of type `T`,
/// all stored inside one physically contiguous memory region.
///
/// Upon destruction:
///
/// - every element is dropped in order;
/// - the contiguous allocation is released automatically.
///
/// The type dereferences directly to `[T]`, allowing it to be used exactly
/// like an ordinary Rust slice.
pub struct ContiguousMemorySlice<T> {
    ptr: ptr::NonNull<T>,
    len: usize,
    mm_caching: MEMORY_CACHING_TYPE,
}


impl<T> ContiguousMemorySlice<T> {
    #[inline(always)]
    pub fn new_uninit(len: usize) -> Option<ContiguousMemorySlice<MaybeUninit<T>>> {
        Self::new_uninit_with_caching_type(DEFAULT_CACHING, len)
    }

    pub fn new_uninit_with_caching_type(mm_caching: MEMORY_CACHING_TYPE, len: usize) -> Option<ContiguousMemorySlice<MaybeUninit<T>>> {
        let ptr = alloc_contiguous_memory(len * size_of::<T>(), mm_caching)? as *mut MaybeUninit<T>;
        let ptr = ptr::NonNull::new(ptr)?;
        unsafe {
            ptr.write_bytes(0, len);
        }
        Some(ContiguousMemorySlice::<MaybeUninit<T>> {
            ptr, len, mm_caching
        })
    }
}


impl<T> ContiguousMemorySlice<MaybeUninit<T>> {
    /// Converts an initialized `MaybeUninit` allocation into its initialized form.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the entire allocation has been fully
    /// initialized.
    ///
    /// Calling this function on uninitialized memory results in undefined
    /// behavior.
    pub unsafe fn assume_init(self) -> ContiguousMemorySlice<T> {
        let ptr = self.ptr.cast();
        let len = self.len;
        let mm_caching = self.mm_caching;
        mem::forget(self);
        ContiguousMemorySlice {
            ptr,
            len,
            mm_caching,
        }
    }
}


impl<T: Clone> ContiguousMemorySlice<T> {
    #[inline(always)]
    pub fn new(val: T, count: usize) -> Option<Self> {
        Self::new_with_caching_type(val, count, DEFAULT_CACHING)
    }

    pub fn new_with_caching_type(val: T, count: usize, caching_type: MEMORY_CACHING_TYPE) -> Option<Self> {
        let mem = alloc_contiguous_memory(count * size_of::<T>(), caching_type)?;
        unsafe {
            let mem_target = mem as *mut T;
            for i in 0..count {
                ptr::write(mem_target.add(i), val.clone());
            }
            let ptr = ptr::NonNull::new(mem_target as _)?;
            Some(Self {
                ptr,
                len: count,
                mm_caching: caching_type,
            })
        }
    }
}


impl<T> ContiguousMemorySlice<T> {
    #[inline(always)]
    pub unsafe fn new_zeroed(len: usize) -> Option<Self> {
        unsafe {
            Self::new_zeroed_with_caching_type(len, DEFAULT_CACHING)
        }
    }

    pub unsafe fn new_zeroed_with_caching_type(len: usize, mm_caching: MEMORY_CACHING_TYPE) -> Option<Self> {
        let mem = alloc_contiguous_memory(len * size_of::<T>(), mm_caching)?;
        unsafe {
            let mem_target = mem as *mut T;
            let ptr = ptr::NonNull::new(mem_target as _)?;
            ptr.write_bytes(0, len);
            Some(Self {
                ptr,
                len,
                mm_caching,
            })
        }
    }

    #[inline(always)]
    pub fn new_with_initializer<F: FnOnce(&mut [MaybeUninit<T>])>(len: usize, initializer: F) -> Option<Self> {
        Self::new_with_initializer_and_caching_type(DEFAULT_CACHING, len, initializer)
    }

    pub fn new_with_initializer_and_caching_type<F: FnOnce(&mut [MaybeUninit<T>])>(mm_caching: MEMORY_CACHING_TYPE, len: usize, initializer: F) -> Option<Self> {
        let mut self_uninit = Self::new_uninit_with_caching_type(mm_caching, len)?;
        initializer(&mut *self_uninit);
        unsafe { Some(self_uninit.assume_init()) }
    }

    pub fn leak(self) -> &'static mut [T] {
        unsafe {
            let ptr = slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            mem::forget(self);
            ptr
        }
    }
}


impl<T> Deref for ContiguousMemorySlice<T> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.ptr.as_ptr(), self.len)
        }
    }
}


impl<T> DerefMut for ContiguousMemorySlice<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len)
        }
    }
}

impl<T> Drop for ContiguousMemorySlice<T> {
    fn drop(&mut self) {
        unsafe {
            let slice = slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            ptr::drop_in_place(slice);
        }
        free_contiguous_memory(self.as_mut_ptr() as _, self.len * size_of::<T>(), self.mm_caching);
    }
}
