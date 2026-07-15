//! Native Windows implementation of the page walker.
//!
//! This module provides a [`MemPhysical`] implementation backed by
//! `MmCopyMemory`, allowing page-table walks using the current system CR3.
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::control::Cr3;
use crate::{MmCopyMemory, _MM_COPY_ADDRESS__bindgen_ty_1, MM_COPY_ADDRESS, MM_COPY_MEMORY_PHYSICAL, NTSTATUS, nt_success, PHYSICAL_ADDRESS};
use crate::memory::pagewalk::{MemPhysical, PageLevel, WalkErr, WalkLevelErr};




/// Reads physical memory using `MmCopyMemory`.
///
/// # Arguments
///
/// * `physical_address` - Physical address to read.
/// * `buffer` - Destination buffer.
///
/// # Errors
///
/// Returns the NTSTATUS returned by `MmCopyMemory` if the operation fails
/// or if fewer bytes than requested were copied.
pub fn read_physical_sys(physical_address: PhysAddr, buffer: &mut [u8]) -> Result<(), NTSTATUS> {
    unsafe {
        let mut copied = 0;
        let src = MM_COPY_ADDRESS {
            __bindgen_anon_1: _MM_COPY_ADDRESS__bindgen_ty_1 {
                PhysicalAddress: PHYSICAL_ADDRESS {
                    QuadPart: physical_address.as_u64() as i64,
                }
            }
        };
        let status = MmCopyMemory(buffer.as_mut_ptr() as *mut _, src, buffer.len() as _, MM_COPY_MEMORY_PHYSICAL, &mut copied);
        if nt_success(status) && copied == buffer.len() as _ {
            Ok(())
        } else {
            Err(status)
        }
    }
}


/// Translates a virtual address using the current system CR3.
///
/// This is a convenience wrapper around
/// [`crate::memory::pagewalk::virtual_to_physical`], performing a full page
/// table walk starting from the current system CR3.
///
/// In most situations where only the physical address of a valid virtual
/// address is needed, [`MmGetPhysicalAddress`] is the preferred and simpler
/// solution. This function is primarily intended for scenarios where an
/// explicit software page walk is required (for example when reusing the page
/// walker implementation or when operating on arbitrary CR3 values).
///
/// # Errors
///
/// Returns [`WalkErr`] if the page walk fails or if reading physical
/// memory through `MmCopyMemory` fails.
pub fn translate_va_to_pa_sys(va: VirtAddr) -> Result<(PhysAddr, PageLevel), WalkErr<NTSTATUS>> {
    super::virtual_to_physical(Cr3::read().0.start_address(), va, read_physical_sys)
}



/// Physical memory reader backed by [`MmCopyMemory`].
///
/// This type implements [`MemPhysical`] using the current system CR3,
/// making it suitable for walking the page tables of the running kernel.
pub struct SysPhysReader {
    cr3: PhysAddr,
}

impl SysPhysReader {
    pub fn new() -> Self {
        Self {
            cr3: Cr3::read().0.start_address(),
        }
    }

    pub fn cr3(&self) -> PhysAddr {
        self.cr3
    }

    pub fn get_address_of_level(&mut self, va: VirtAddr, level: PageLevel) -> Result<PhysAddr, WalkLevelErr<NTSTATUS>> {
        MemPhysical::get_address_of_level(self, self.cr3, va, level)
    }

    pub fn virtual_to_physical(&mut self, va: VirtAddr) -> Result<(PhysAddr, PageLevel), WalkErr<NTSTATUS>> {
        MemPhysical::virtual_to_physical(self, self.cr3, va)
    }
}

impl MemPhysical for SysPhysReader {
    type E = NTSTATUS;
    fn read_physical(&mut self, phys: PhysAddr, buf: &mut [u8]) -> Result<(), Self::E> {
        read_physical_sys(phys, buf)
    }
}