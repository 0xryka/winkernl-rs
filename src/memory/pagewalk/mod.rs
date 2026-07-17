//! Page table walking utilities.
//!
//! This module provides architecture-independent helpers for walking
//! x86-64 page tables from a physical memory source.
//!
//! Unlike operating system APIs, these routines operate directly on the
//! paging hierarchy by reading page table entries from physical memory.
//! They therefore work in environments where only raw physical memory
//! access is available, such as hypervisors, kernel drivers or memory
//! acquisition tools.
//!
//! # Features
//!
//! The module supports:
//!
//! - translating virtual addresses to physical addresses,
//! - retrieving the physical address of any paging structure,
//! - reading individual page table entries,
//! - walking paging structures through the [`MemPhysical`] trait,
//! - transparent handling of 2 MiB and 1 GiB huge pages during address
//!   translation.
//!
//! # Paging hierarchy
//!
//! ```text
//! Virtual Address
//!        │
//!        ▼
//!      PML4
//!        │
//!        ▼
//!      PDPT
//!        │
//!        ▼
//!        PD
//!        │
//!        ▼
//!        PT
//!        │
//!        ▼
//!   Physical Page
//! ```
//!
//! Every helper accepts the physical address of the PML4 table and a
//! virtual address to resolve.
pub mod winimpl;

use core::mem::MaybeUninit;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::page_table::PageTableEntry;
use x86_64::structures::paging::{PageTableFlags, PageTableIndex};



/// Errors that may occur while translating a virtual address.
///
/// These errors distinguish between failures caused by invalid page table
/// entries and failures originating from the physical memory backend.
pub enum WalkErr<E> {
    InvalidPml4(PageTableEntry),
    InvalidPdpt(PageTableEntry),
    InvalidPde(PageTableEntry),
    InvalidPte(PageTableEntry),
    ReadErr(E),
}

impl<E> From<E> for WalkErr<E> {
    fn from(e: E) -> Self {
        Self::ReadErr(e)
    }
}


/// Identifies one level of the x86-64 paging hierarchy.
///
/// This enumeration is primarily used by
/// [`get_address_of_level`] to specify which paging structure
/// should be returned.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PageLevel {
    Pml4,
    Pdpt,
    Pd,
    Pt,
    Phys,
}


/// Errors returned while retrieving the address of a paging structure.
///
/// Unlike [`WalkErr`], this error type also reports when the requested
/// paging level cannot exist because a huge page terminated the walk
/// earlier.
pub enum WalkLevelErr<E> {
    WalkErr(WalkErr<E>),
    HugePageBeforeLevel,
}

impl<E> From<E> for WalkLevelErr<E> {
    fn from(e: E) -> Self {
        Self::WalkErr(WalkErr::ReadErr(e))
    }
}

impl<E> From<WalkErr<E>> for WalkLevelErr<E> {
    fn from(e: WalkErr<E>) -> Self {
        Self::WalkErr(e)
    }
}



/// Abstraction over a physical memory reader.
///
/// Types implementing this trait only need to provide
/// [`MemPhysical::read_physical`].
///
/// Every other page walking routine is implemented as a default method,
/// allowing the same translation logic to be reused regardless of the
/// underlying physical memory backend.
///
/// Typical implementations include:
///
/// - kernel drivers,
/// - hypervisors,
/// - memory dump readers,
/// - DMA devices.
pub trait MemPhysical {
    type E;

    /// Reads raw bytes from physical memory.
    ///
    /// # Arguments
    ///
    /// * `phys` - Physical address to read.
    /// * `buf` - Destination buffer.
    ///
    /// # Errors
    ///
    /// Returns an implementation-defined error if the physical read fails.
    fn read_physical(&mut self, phys: PhysAddr, buf: &mut [u8]) -> Result<(), Self::E>;


    /// Reads a page table entry from a paging structure.
    ///
    /// The entry is read directly from physical memory using the supplied
    /// callback.
    ///
    /// # Arguments
    ///
    /// * `table` - Physical address of the page table.
    /// * `idx` - Entry index within the table.
    ///
    /// # Returns
    ///
    /// Returns the decoded [`PageTableEntry`] on success.
    #[inline]
    fn read_page_table_entry_e(&mut self, table: PhysAddr, idx: PageTableIndex) -> Result<PageTableEntry, WalkErr<Self::E>> {
        read_page_table_entry_e(table, idx, |phys_addr, buffer| self.read_physical(phys_addr, buffer))
    }

    /// Returns the physical address of a paging structure entry.
    ///
    /// Rather than translating the virtual address completely, this function
    /// stops once the requested paging level has been reached.
    ///
    /// For example, requesting [`PageLevel::Pd`] returns the physical
    /// address of the PDE corresponding to the supplied virtual address.
    ///
    /// # Errors
    ///
    /// Returns [`WalkLevelErr::HugePageBeforeLevel`] if a huge page prevents
    /// the requested paging level from existing.
    #[inline]
    fn get_address_of_level(&mut self, pml4_phys: PhysAddr, va: VirtAddr, level: PageLevel) -> Result<PhysAddr, WalkLevelErr<Self::E>> {
        get_address_of_level(pml4_phys, va, level, |phys_addr, buffer| self.read_physical(phys_addr, buffer))
    }

    /// Translates a virtual address into a physical address.
    ///
    /// The page tables are walked starting from the supplied PML4 physical
    /// address.
    ///
    /// Both 2 MiB and 1 GiB huge pages are handled transparently.
    ///
    /// # Returns
    ///
    /// Returns the resolved physical address or an appropriate
    /// [`WalkErr`] if translation failed.
    #[inline]
    fn virtual_to_physical(&mut self, pml4_phys: PhysAddr, va: VirtAddr) -> Result<(PhysAddr, PageLevel), WalkErr<Self::E>> {
        virtual_to_physical(pml4_phys, va, |phys_addr, buffer| self.read_physical(phys_addr, buffer))
    }
}




/// Reads a single page table entry from physical memory.
///
/// This helper reads the entry located at `table[idx]` using the supplied
/// physical memory reader.
///
/// # Arguments
///
/// * `table` - Physical address of the page table.
/// * `idx` - Index of the entry within the table.
/// * `read_physical` - Callback used to read physical memory.
///
/// # Returns
///
/// Returns the decoded [`PageTableEntry`] on success.
///
/// # Errors
///
/// Returns [`WalkErr::ReadErr`] if the underlying physical read fails.
#[inline(always)]
pub fn read_page_table_entry_e<E, F: FnMut(PhysAddr, &mut [u8]) -> Result<(), E>>(table: PhysAddr, idx: PageTableIndex, mut read_physical: F) -> Result<PageTableEntry, WalkErr<E>> {
    read_physical_memory_t(table + (u64::from(idx) * 8), read_physical).map_err(WalkErr::ReadErr)
}


pub fn read_physical_memory_t<T, E, F: FnMut(PhysAddr, &mut [u8]) -> Result<(), E>>(phys: PhysAddr, mut read_physical: F) -> Result<T, E> {
    let mut data = MaybeUninit::<T>::uninit();
    unsafe {
        let buf = core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, size_of::<T>());
        read_physical(phys, buf)?;
        Ok(data.assume_init())
    }
}

/// Returns the physical address of the page table entry (PTE)
/// corresponding to a virtual address.
///
/// This is a convenience wrapper around [`get_address_of_level`] with
/// [`PageLevel::Pt`].
///
/// It is primarily intended for scenarios where the caller wants to access
/// or modify the PTE itself (for example, changing page permissions or
/// inspecting page table flags) rather than translating the virtual address
/// to its backing physical address.
///
/// # Arguments
///
/// * `pml4_phys` - Physical address of the target PML4.
/// * `va` - Virtual address whose PTE should be located.
/// * `read_physical` - Callback used to access physical memory.
///
/// # Errors
///
/// Returns [`WalkLevelErr`] if page table walking fails or if a large page
/// prevents reaching the PT level.
#[inline(always)]
pub fn get_pte<E, F: FnMut(PhysAddr, &mut [u8]) -> Result<(), E>>(pml4_phys: PhysAddr, va: VirtAddr, read_physical: F) -> Result<PhysAddr, WalkLevelErr<E>> {
    get_address_of_level(pml4_phys, va, PageLevel::Pt, read_physical)
}


/// Returns the physical address of a page-table entry at the requested level.
///
/// Unlike [`virtual_to_physical`], this function does not perform a full
/// virtual-to-physical translation. Instead, it returns the physical address
/// of the page-table entry corresponding to the requested paging level.
///
/// This can be used to inspect or modify paging structures directly.
///
/// # Arguments
///
/// * `pml4_phys` - Physical address of the root PML4 table.
/// * `va` - Virtual address whose translation should be walked.
/// * `level` - Paging level to stop at.
/// * `read_physical` - Callback used to read physical memory.
///
/// # Returns
///
/// Returns the physical address of the requested page-table entry.
///
/// # Errors
///
/// Returns:
///
/// * [`WalkErr`] if an entry cannot be read or is not present.
/// * [`WalkLevelErr::HugePageBeforeLevel`] if a large page mapping is
///   encountered before reaching the requested level.
pub fn get_address_of_level<E, F: FnMut(PhysAddr, &mut [u8]) -> Result<(), E>>(pml4_phys: PhysAddr, va: VirtAddr, level: PageLevel, mut read_physical: F) -> Result<PhysAddr, WalkLevelErr<E>> {
    let pml4_idx = va.p4_index();
    let addr = PhysAddr::new(pml4_phys.as_u64() + u64::from(pml4_idx) * 8);
    if level == PageLevel::Pml4 {
        return Ok(addr);
    }
    let pml4e: PageTableEntry = read_physical_memory_t(addr, &mut read_physical)?;
    if !pml4e.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkLevelErr::WalkErr(WalkErr::InvalidPml4(pml4e)));
    }

    let addr = pml4e.addr() + (u64::from(va.p3_index()) * 8);
    if level == PageLevel::Pdpt {
        return Ok(addr);
    }

    let pdpte: PageTableEntry = read_physical_memory_t(addr, &mut read_physical)?;
    if !pdpte.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkLevelErr::WalkErr(WalkErr::InvalidPdpt(pdpte)));
    }
    if pdpte.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Err(WalkLevelErr::HugePageBeforeLevel);
    }


    let addr = pdpte.addr() + (u64::from(va.p2_index()) * 8);
    if level == PageLevel::Pd {
        return Ok(addr);
    }

    let pde: PageTableEntry = read_physical_memory_t(addr, &mut read_physical)?;
    if !pde.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkLevelErr::WalkErr(WalkErr::InvalidPde(pde)));
    }
    if pde.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Err(WalkLevelErr::HugePageBeforeLevel);
    }

    let addr = pde.addr() + (u64::from(va.p1_index()) * 8);
    if level == PageLevel::Pt {
        return Ok(addr);
    }

    let pte: PageTableEntry = read_physical_memory_t(addr, &mut read_physical)?;
    if !pte.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkLevelErr::WalkErr(WalkErr::InvalidPte(pte)));
    }

    Ok(pte.addr() + (va.as_u64() & 0xFFF))
}



/// Translates a virtual address into its corresponding physical address.
///
/// This function performs a complete x86-64 page-table walk starting from the
/// supplied PML4 physical address. Standard 4 KiB pages as well as 2 MiB and
/// 1 GiB huge pages are supported.
///
/// Physical memory accesses are delegated to the provided `read_physical`
/// callback, making this function independent of any particular physical
/// memory access mechanism.
///
/// # Arguments
///
/// * `pml4_phys` - Physical address of the root PML4 table.
/// * `va` - Virtual address to translate.
/// * `read_physical` - Callback used to read physical memory while walking the
///   page tables.
///
/// # Returns
///
/// On success, returns a tuple containing:
///
/// * The translated physical address corresponding to `va`.
/// * The [`PageLevel`] at which the translation terminated:
///   - [`PageLevel::Pdpt`] for a 1 GB page.
///   - [`PageLevel::Pd`] for a 2 MB page.
///   - [`PageLevel::Phys`] for a standard 4 KB page.
///
/// # Errors
///
/// Returns:
///
/// * [`WalkErr::InvalidPml4`] if the selected PML4 entry is not present.
/// * [`WalkErr::InvalidPdpt`] if the selected PDPT entry is not present.
/// * [`WalkErr::InvalidPde`] if the selected page directory entry is not present.
/// * [`WalkErr::InvalidPte`] if the selected page table entry is not present.
/// * [`WalkErr::ReadErr`] if the `read_physical` callback fails while reading a
///   page-table entry.
#[inline(always)]
pub fn virtual_to_physical<E, F: FnMut(PhysAddr, &mut [u8]) -> Result<(), E>>(pml4_phys: PhysAddr, va: VirtAddr, mut read_physical: F) -> Result<(PhysAddr, PageLevel), WalkErr<E>> {
    let pml4e = read_page_table_entry_e(pml4_phys, va.p4_index(), &mut read_physical)?;
    if !pml4e.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkErr::InvalidPml4(pml4e));
    }

    let pdpte = read_page_table_entry_e(pml4e.addr(), va.p3_index(), &mut read_physical)?;
    if !pdpte.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkErr::InvalidPdpt(pdpte));
    }
    if pdpte.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Ok((pdpte.addr() + (va.as_u64() & 0x3FFFFFFF), PageLevel::Pdpt));
    }

    let pde = read_page_table_entry_e(pdpte.addr(), va.p2_index(), &mut read_physical)?;
    if !pde.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkErr::InvalidPde(pde));
    }
    if pde.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Ok((pde.addr() + (va.as_u64() & 0x1FFFFF), PageLevel::Pd));
    }

    let pte = read_page_table_entry_e(pde.addr(), va.p1_index(), &mut read_physical)?;
    if !pte.flags().contains(PageTableFlags::PRESENT) {
        return Err(WalkErr::InvalidPte(pte));
    }

    Ok((pte.addr() + (va.as_u64() & 0xFFF), PageLevel::Phys))
}