//! Windows kernel pool allocation.
//!
//! This module provides safe ownership wrappers around memory allocated from
//! the Windows kernel pool.
//!
//! Unlike contiguous allocations, pool allocations do **not** guarantee that
//! the underlying physical pages are adjacent. They are the preferred choice
//! for most kernel allocations due to their flexibility and lower allocation
//! cost.
//!
//! The caller may select the desired [`POOL_TYPE`]
//! (for example `NonPagedPool` or `PagedPool`) depending on where the memory
//! must reside.
//!
//! ## Allocation types
//!
//! Two owning wrappers are provided.
//!
//! ### `PoolMemory<T>`
//!
//! Owns a single object allocated from a kernel pool.
//! It behaves similarly to `Box<T>`.
//!
//! ### `AllocPoolSlice<T>`
//!
//! Owns a runtime-sized array allocated from a kernel pool.
//! It behaves similarly to `Box<[T]>`.
//!
//! ## Automatic resource management
//!
//! Both wrappers automatically:
//!
//! - allocate memory from the requested kernel pool;
//! - call the destructor of every initialized object;
//! - release the allocation with `ExFreePool` when dropped.
//!
//! ## Initialization
//!
//! Several initialization strategies are available:
//!
//! - `new()`
//! - `new_zeroed()`
//! - `new_uninit()`
//! - `new_with_initializer()`
//!
//! The `new_uninit*` constructors return wrappers over
//! `MaybeUninit<T>`, allowing objects to be initialized directly in their
//! final memory location.
//!
//! Once every object has been initialized,
//! `unsafe assume_init()` converts the allocation into its initialized form.
//!
//! ## Pool type
//!
//! Every allocation is performed using a
//! [`POOL_TYPE`].
//!
//! By default, allocations use `NonPagedPool`, although constructors ending
//! with `_with_pool_type` allow selecting another pool.
//!
//! ## Examples
//!
//! Allocate a single object:
//!
//! ```no_run
//! use winkernl_rs::kalloc::pool::PoolMemory;
//!
//! let value = PoolMemory::new(123u32).unwrap();
//! assert_eq!(*value, 123);
//! ```
//!
//! Allocate a runtime-sized array:
//!
//! ```no_run
//! use winkernl_rs::kalloc::pool::PoolMemorySlice;
//!
//! let buffer = unsafe {
//!     PoolMemorySlice::<u8>::new_zeroed(4096).unwrap()
//! };
//!
//! assert_eq!(buffer.len(), 4096);
//! ```
//!
//! Initialize directly inside the allocation:
//!
//! ```no_run
//! use core::mem::MaybeUninit;
//! use winkernl_rs::kalloc::pool::PoolMemory;
//!
//! let value = PoolMemory::new_with_initializer(
//!     |slot: &mut MaybeUninit<u64>| {
//!         slot.write(42);
//!     }
//! ).unwrap();
//! ```
use core::{mem, ptr, slice};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use crate::memory;
use crate::memory::allocator::{alloc_pool, alloc_pool_t, free_pool};
use crate::sys::POOL_TYPE;



const DEFAULT_POOL_TYPE: POOL_TYPE = POOL_TYPE::NonPagedPool;

pub struct PoolMemory<T> {
    mem: NonNull<T>,
}


impl<T> PoolMemory<T> {
    /// Creates a new, uninitialized pool memory block using the default pool type.
    ///
    /// This is a convenience wrapper around [`Self::new_uninit_with_pool_type`]
    /// using [`DEFAULT_POOL_TYPE`].
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<MaybeUninit<T>>)`] pointing to the newly allocated
    /// uninitialized memory, or [`None`] if the allocation fails.
    pub fn new_uninit() -> Option<PoolMemory<MaybeUninit<T>>> {
        Self::new_uninit_with_pool_type(DEFAULT_POOL_TYPE)
    }

    /// Creates a new, uninitialized pool memory block with a specified pool type.
    ///
    /// This method allocates memory from the kernel pool specified by `pool_type`.
    /// The returned memory is wrapped in `MaybeUninit<T>` because it is not yet initialized,
    /// preventing undefined behavior from reading uninitialized data.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<MaybeUninit<T>>)`] containing a non-null pointer to the
    /// allocated space, or [`None`] if the pool allocation ([`alloc_pool_t`]) fails or returns
    /// a null pointer.
    pub fn new_uninit_with_pool_type(pool_type: POOL_TYPE) -> Option<PoolMemory<MaybeUninit<T>>> {
        let mem = alloc_pool_t::<MaybeUninit<T>>(pool_type)?;
        let mem = NonNull::new(mem)?;
        Some(PoolMemory { mem })
    }
}


impl<T> PoolMemory<MaybeUninit<T>> {
    /// Assumes the memory block is fully initialized and transitions ownership
    /// to a type-safe [`PoolMemory<T>`].
    ///
    /// This method bypasses the compiler's initialization checks. It is a zero-cost
    /// cast that consumes the uninitialized pool memory wrapper and returns an initialized one.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the memory pointed to by this block has been
    /// **fully and correctly initialized**. Calling this method on uninitialized memory
    /// and subsequently reading from or dropping the returned [`PoolMemory<T>`]
    /// will result in **undefined behavior**.
    ///
    /// # Examples
    ///
    /// ```rs
    /// let mut pool_mem = PoolMemory::<u32>::new_uninit()?;
    ///
    /// // Safe initialization of the raw memory
    /// unsafe {
    ///     pool_mem.as_mut_ptr().write(42);
    /// }
    ///
    /// // Transition to fully initialized state
    /// let initialized_mem = unsafe { pool_mem.assume_init() };
    /// ```
    pub unsafe fn assume_init(self) -> PoolMemory<T> {
        let mem = self.mem.cast();
        mem::forget(self);
        PoolMemory { mem }
    }
}


impl<T> PoolMemory<T> {
    /// Allocates pool memory using the default pool type and initializes it with `val`.
    ///
    /// This is a convenience wrapper around [`Self::new_with_pool_type`] using
    /// [`DEFAULT_POOL_TYPE`].
    ///
    /// # Parameters
    ///
    /// * `val` - The value to write into the newly allocated pool memory.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>)`] containing the initialized allocation, or
    /// [`None`] if the allocation fails.
    pub fn new(val: T) -> Option<Self> {
        Self::new_with_pool_type(val, DEFAULT_POOL_TYPE)
    }


    /// Allocates pool memory of the specified type and initializes it with `val`.
    ///
    /// Memory is allocated from the kernel pool designated by `pool_type`, then
    /// `val` is moved into the newly allocated storage.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to store in the allocation.
    /// * `pool_type` - The kernel pool from which memory should be allocated.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>)`] containing the initialized allocation, or
    /// [`None`] if the allocation fails.
    pub fn new_with_pool_type(val: T, pool_type: POOL_TYPE) -> Option<Self> {
        unsafe {
            let mem = NonNull::new(alloc_pool_t(pool_type)?)?;
            mem.write(val);
            Some(Self { mem })
        }
    }

    /// Allocates zero-initialized pool memory using the paged pool.
    ///
    /// This is a convenience wrapper around
    /// [`Self::new_zeroed_with_pool_type`] using
    /// [`POOL_TYPE::PagedPool`].
    ///
    /// # Safety
    ///
    /// Zero-initialization is only valid for types where an all-zero byte
    /// pattern represents a valid value. Calling this function for a type that
    /// does not permit an all-zero representation results in undefined behavior.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>`] containing the zero-initialized memory,
    /// or [`None`] if allocation fails.
    pub unsafe fn new_zeroed() -> Option<Self> {
        unsafe {
            Self::new_zeroed_with_pool_type(POOL_TYPE::PagedPool)
        }
    }

    /// Allocates zero-initialized pool memory from the specified pool.
    ///
    /// The allocated memory is filled with zero bytes before being considered
    /// initialized.
    ///
    /// # Safety
    ///
    /// The caller must ensure that an all-zero byte pattern is a valid
    /// representation of `T`. This is not true for every type.
    ///
    /// # Parameters
    ///
    /// * `pool_type` - The kernel pool from which memory should be allocated.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>`] containing the zero-initialized
    /// allocation, or [`None`] if allocation fails.
    pub unsafe fn new_zeroed_with_pool_type(pool_type: POOL_TYPE) -> Option<Self> {
        unsafe {
            let mem = NonNull::new(alloc_pool_t(pool_type)?)?;
            mem.write_bytes(0, 1);
            Some(Self { mem })
        }
    }

    /// Allocates uninitialized pool memory using the default pool type and
    /// initializes it through a user-provided closure.
    ///
    /// This is a convenience wrapper around
    /// [`Self::new_with_initializer_and_pool_type`] using
    /// [`DEFAULT_POOL_TYPE`].
    ///
    /// The closure receives a mutable reference to the underlying
    /// [`MaybeUninit<T>`], allowing the value to be initialized in place.
    ///
    /// # Parameters
    ///
    /// * `initializer` - Closure responsible for initializing the allocation.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>`] if allocation succeeds, or [`None`] if
    /// allocation fails.
    pub fn new_with_initializer<F: FnOnce(&mut MaybeUninit<T>)>(initializer: F) -> Option<Self> {
        Self::new_with_initializer_and_pool_type(DEFAULT_POOL_TYPE, initializer)
    }

    /// Allocates uninitialized pool memory from the specified pool and
    /// initializes it through a user-provided closure.
    ///
    /// The closure is given mutable access to the underlying
    /// [`MaybeUninit<T>`], allowing the value to be constructed directly in the
    /// allocated storage without requiring an intermediate move.
    ///
    /// # Parameters
    ///
    /// * `pool_type` - The kernel pool from which memory should be allocated.
    /// * `initializer` - Closure responsible for initializing the allocation.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemory<T>>`] if allocation succeeds, or [`None`] if
    /// allocation fails.
    ///
    ///
    /// # Safety
    ///
    /// The initializer **must fully initialize** the provided
    /// [`MaybeUninit<T>`]. Failing to do so causes the subsequent internal call
    /// to [`MaybeUninit::assume_init`] to invoke undefined behavior.
    pub fn new_with_initializer_and_pool_type<F: FnOnce(&mut MaybeUninit<T>)>(pool_type: POOL_TYPE, initializer: F) -> Option<Self> {
        let mut self_uninit = Self::new_uninit_with_pool_type(pool_type)?;
        initializer(&mut *self_uninit);
        unsafe {
            Some(self_uninit.assume_init())
        }
    }

    /// Leaks the allocation and returns a mutable reference with `'static`
    /// lifetime.
    ///
    /// The underlying pool allocation is intentionally forgotten and will never
    /// be automatically freed. The caller becomes responsible for managing the
    /// lifetime of the returned reference.
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to the contained value with `'static`
    /// lifetime.
    ///
    /// # Memory leaks
    ///
    /// This function intentionally leaks both the allocation and the
    /// [`PoolMemory`] wrapper. Unless the memory is later reclaimed manually,
    /// it will remain allocated until the system unloads the driver or
    /// terminates.
    pub fn leak(self) -> &'static mut T {
        let ptr = self.mem.as_ptr();
        mem::forget(self);
        unsafe { &mut *ptr }
    }
}



impl<T> Deref for PoolMemory<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.mem.as_ptr()) }
    }
}


impl<T> DerefMut for PoolMemory<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.mem.as_ptr()) }
    }
}


impl<T> Drop for PoolMemory<T> {
    fn drop(&mut self) {
        unsafe {
            self.mem.as_ptr().drop_in_place();
        }
        free_pool(self.mem.as_ptr() as _);
    }
}



/// A dynamically-sized allocation backed by kernel pool memory.
///
/// `PoolMemorySlice<T>` owns a contiguous allocation containing `len` elements
/// of type `T`. Unlike [`PoolMemory<T>`], which manages a single value, this
/// type is designed for dynamically-sized arrays and slices whose length is
/// determined at runtime.
///
/// The allocation is automatically released when the `PoolMemorySlice` is
/// dropped, unless ownership is intentionally relinquished through
/// [`PoolMemorySlice::leak`].
///
/// Uninitialized allocations are represented by
/// `PoolMemorySlice<MaybeUninit<T>>`, preventing accidental access to
/// uninitialized elements until they are explicitly initialized and converted
/// with [`PoolMemorySlice::assume_init`].
pub struct PoolMemorySlice<T> {
    ptr: NonNull<T>,
    len: usize,
    pool_type: POOL_TYPE,
}


impl<T: Clone> PoolMemorySlice<T> {
    /// Allocates a slice in the default pool and initializes every element with a
    /// clone of `val`.
    ///
    /// This is a convenience wrapper around [`Self::new_with_pool_type`] using
    /// [`DEFAULT_POOL_TYPE`].
    ///
    /// # Parameters
    ///
    /// * `val` - Value to clone into every element.
    /// * `count` - Number of elements to allocate.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<T>>`] on success, or [`None`] if allocation
    /// fails.
    pub fn new(val: T, count: usize) -> Option<Self> {
        Self::new_with_pool_type(val, count, POOL_TYPE::PagedPool)
    }

    /// Allocates a slice from the specified kernel pool and initializes every
    /// element with a clone of `val`.
    ///
    /// Each element is individually constructed by calling [`Clone::clone`] on
    /// `val`.
    ///
    /// # Parameters
    ///
    /// * `val` - Value to clone into every element.
    /// * `count` - Number of elements to allocate.
    /// * `pool_type` - Kernel pool from which the allocation should be made.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<T>>`] on success, or [`None`] if allocation
    /// fails.
    pub fn new_with_pool_type(val: T, count: usize, pool_type: POOL_TYPE) -> Option<Self> {
        let mem = alloc_pool(pool_type, count * size_of::<T>())?;
        let ptr = NonNull::new(mem as *mut T)?;
        for i in 0..count {
            unsafe {
                ptr.add(i).write(val.clone());
            }
        }
        Some(Self {
            ptr,
            len: count,
            pool_type,
        })
    }
}


impl<T> PoolMemorySlice<T> {
    /// Allocates an uninitialized slice using the default pool type.
    ///
    /// This is a convenience wrapper around
    /// [`Self::new_uninit_with_pool_type`] using [`DEFAULT_POOL_TYPE`].
    ///
    /// The returned allocation is wrapped in [`MaybeUninit`] to prevent
    /// accidentally reading uninitialized elements.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<MaybeUninit<T>>>)`] on success, or [`None`]
    /// if allocation fails.
    #[inline(always)]
    pub fn new_uninit(len: usize) -> Option<PoolMemorySlice<MaybeUninit<T>>> {
        Self::new_uninit_with_pool_type(len, POOL_TYPE::PagedPool)
    }

    /// Allocates an uninitialized slice from the specified kernel pool.
    ///
    /// Every element is represented as [`MaybeUninit<T>`] and must be fully
    /// initialized before calling [`PoolMemorySlice::assume_init`].
    ///
    /// # Parameters
    ///
    /// * `len` - Number of elements to allocate.
    /// * `pool_type` - Kernel [`POOL_TYPE`] from which memory should be allocated.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<MaybeUninit<T>>>`] on success, or [`None`]
    /// if allocation fails.
    pub fn new_uninit_with_pool_type(len: usize, pool_type: POOL_TYPE) -> Option<PoolMemorySlice<MaybeUninit<T>>> {
        let mem = alloc_pool(pool_type, size_of::<T>() * len)? as *mut MaybeUninit<T>;
        let ptr = NonNull::new(mem)?;
        unsafe {
            ptr.write_bytes(0, len);
        }
        Some(PoolMemorySlice::<MaybeUninit<T>> {
            pool_type, len, ptr,
        })
    }
}

impl<T> PoolMemorySlice<MaybeUninit<T>> {
    /// Assumes that every element of the slice has been fully initialized and
    /// converts the allocation into `PoolMemorySlice<T>`.
    ///
    /// This is a zero-cost conversion.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that **every element** of the allocation has been
    /// completely initialized. Calling this function when one or more elements are
    /// still uninitialized results in undefined behavior.
    pub unsafe fn assume_init(self) -> PoolMemorySlice<T> {
        let ptr = self.ptr.cast();
        let len = self.len;
        let pool_type = self.pool_type;
        mem::forget(self);
        PoolMemorySlice {
            ptr,
            len,
            pool_type,
        }
    }
}

impl<T> PoolMemorySlice<T> {
    /// Allocates a zero-initialized slice using the default pool type.
    ///
    /// This is a convenience wrapper around
    /// [`Self::new_zeroed_with_pool_type`].
    ///
    /// # Safety
    ///
    /// An all-zero byte pattern must be a valid representation for `T`.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<T>>`] on success, or [`None`] if allocation
    /// fails.
    #[inline(always)]
    pub unsafe fn new_zeroed(count: usize) -> Option<Self> {
        unsafe {
            Self::new_zeroed_with_pool_type(count, POOL_TYPE::PagedPool)
        }
    }

    /// Allocates a zero-initialized slice from the specified kernel pool.
    ///
    /// # Safety
    ///
    /// The caller must ensure that an all-zero byte pattern is a valid
    /// representation for `T`.
    ///
    /// # Parameters
    ///
    /// * `count` - Number of elements to allocate.
    /// * `pool_type` - Kernel pool from which memory should be allocated.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<T>>`] on success, or [`None`] if allocation
    /// fails.
    pub unsafe fn new_zeroed_with_pool_type(count: usize, pool_type: POOL_TYPE) -> Option<Self> {
        let mem = alloc_pool(pool_type, count * size_of::<T>())?;
        let ptr = NonNull::new(mem as *mut T)?;
        unsafe {
            ptr.write_bytes(0, count);
        }
        Some(Self {
            ptr,
            len: count,
            pool_type,
        })
    }

    /// Allocates an uninitialized slice and initializes it through a user-provided
    /// closure.
    ///
    /// The closure receives a mutable slice of [`MaybeUninit<T>`], allowing each
    /// element to be constructed directly in place.
    ///
    /// This is a convenience wrapper around
    /// [`Self::new_with_initializer_and_pool_type`] using
    /// [`DEFAULT_POOL_TYPE`].
    #[inline(always)]
    pub fn new_with_initializer<F: FnOnce(&mut [MaybeUninit<T>])>(len: usize, initializer: F) -> Option<Self> {
        Self::new_with_initializer_and_pool_type(len, DEFAULT_POOL_TYPE, initializer)
    }

    /// Allocates an uninitialized slice from the specified kernel pool and
    /// initializes it through a user-provided closure.
    ///
    /// The initializer receives a mutable slice of [`MaybeUninit<T>`], allowing
    /// each element to be constructed directly in the allocated storage.
    ///
    /// # Parameters
    ///
    /// * `len` - Number of elements to allocate.
    /// * `pool_type` - Kernel pool from which memory should be allocated.
    /// * `initializer` - Closure responsible for initializing every element.
    ///
    /// # Returns
    ///
    /// Returns [`Some(PoolMemorySlice<T>>)`] on success, or [`None`] if allocation
    /// fails.
    ///
    /// # Safety
    ///
    /// The initializer **must initialize every element** of the provided slice
    /// before returning. Failing to do so results in undefined behavior because the
    /// allocation is internally converted using
    /// [`MaybeUninit::assume_init`].
    pub fn new_with_initializer_and_pool_type<F: FnOnce(&mut [MaybeUninit<T>])>(len: usize, pool_type: POOL_TYPE, initializer: F) -> Option<Self> {
        let mut self_uninit = Self::new_uninit_with_pool_type(len, pool_type)?;
        initializer(&mut *self_uninit);
        unsafe {
            Some(self_uninit.assume_init())
        }
    }

    /// Leaks the allocation and returns a mutable slice with `'static` lifetime.
    ///
    /// The allocation is intentionally forgotten and will no longer be
    /// automatically released. The caller becomes responsible for managing the
    /// lifetime of the returned slice.
    ///
    /// # Returns
    ///
    /// A mutable slice covering the entire allocation.
    ///
    /// # Memory leaks
    ///
    /// Unless the allocation is later reclaimed manually, the underlying kernel
    /// pool memory will remain allocated for the remainder of the driver's
    /// lifetime.
    #[allow(warnings)]
    pub fn leak(self) -> &'static mut [T] {
        unsafe {
            let ptr = slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            mem::forget(self);
            ptr
        }
    }
}

impl<T> Deref for PoolMemorySlice<T> {
    type Target = [T];
    /// Returns the allocation as an immutable slice.
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> DerefMut for PoolMemorySlice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}




impl<T> Drop for PoolMemorySlice<T> {
    fn drop(&mut self) {
        unsafe {
            let slice = slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            ptr::drop_in_place(slice);
        }
        free_pool(self.ptr.as_ptr() as _);
    }
}