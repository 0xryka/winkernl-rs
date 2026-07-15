//! Raw Windows kernel bindings.
//!
//! This module exposes low-level Rust bindings to the Windows kernel
//! interfaces defined by the WDK, including declarations originating
//! from headers such as **ntddk.h**, **ntifs.h** and **ntstatus.h**.
//!
//! The bindings are intentionally thin and closely match the native C
//! definitions, making them suitable for interfacing directly with the
//! Windows kernel.
//!
//! Most users should prefer the safe wrappers provided by other modules
//! in this crate (such as `memory`, `process` or `kalloc`) instead of
//! interacting with these raw APIs directly.
pub mod ntoskrnl;
pub mod ntstatus;

pub use ntoskrnl::*;
pub use ntstatus::*;