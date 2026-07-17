//! Memory management utilities.
//!
//! This module groups the crate's memory-related facilities, including:
//!
//! - kernel pool and contiguous memory allocation;
//! - virtual and physical memory access helpers;
//! - page-table walking utilities;
//! - kernel module lookup and exported routine resolution;
//! - low-level read/write primitives.
//!
//! # Modules
//!
//! - [`allocator`] provides kernel allocation utilities and safe RAII
//!   wrappers around Windows pool allocators.
//! - [`module`] provides helpers for locating loaded kernel modules and
//!   resolving exported routines.
//! - [`pagewalk`] implements generic x86-64 page-table walking utilities,
//!   including virtual-to-physical translation.
//! - [`rw`] contains helpers for reading from and writing to kernel memory.
pub mod module;
pub mod rw;
pub mod pagewalk;
pub mod allocator;
pub mod mdl;

/// Searches for a byte pattern within a memory region.
///
/// The pattern supports wildcard bytes by using `None`.
///
/// # Arguments
///
/// * `base` - Base address of the memory region.
/// * `img_size` - Size of the region to scan.
/// * `pattern` - Pattern to search. Each element is either:
///   - `Some(byte)` to match an exact value;
///   - `None` to match any byte.
///
/// # Returns
///
/// Returns the virtual address of the first matching occurrence, or `None`
/// if the pattern cannot be found.
///
/// # Safety
///
/// The caller must ensure that the range `[base, base + img_size)` is
/// readable for the duration of the search.
pub fn pattern_search(base: u64, img_size: usize, pattern: &[Option<u8>]) -> Option<u64> {
    unsafe {
        let plen = pattern.len();
        if plen == 0 || img_size < plen {
            return None;
        }
        let last = img_size - plen;
        for i in 0..=last {
            let start = (base + i as u64) as *const u8;
            let mut finded = true;

            for (j, pat) in pattern.iter().enumerate() {
                let mem_byte = *start.add(j);
                match pat {
                    Some(b) => {
                        if mem_byte != *b {
                            finded = false;
                            break;
                        }
                    }
                    None => {},
                }
            }

            if finded {
                return Some(base + i as u64);
            }
        }
        None
    }
}
