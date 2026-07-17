# winkernl-rs

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows%20Kernel-blue.svg)]()

> High-level Rust abstractions for Windows kernel-mode development

`winkernel-rs` is a Rust library for Windows kernel-mode (Ring 0) development that provides safe, ergonomic abstractions over native WDK API while remaining faithful to the underlying Windows kernel programming model

Rather than exposing only raw FFI bindings, the library offers reusable building blocks for common kernel facilities such as pool allocations, page-table walking, Memory Descriptor Lists (MDL), module resolution, process structures, inline hooking, and other low-level mechanisms The goal is to reduce boilerplate and improve safety without sacrificing performance, flexibility, or understanding of the underlying system

The project is designed to support as many Ring 0 environments as possible Whether it is used from a traditional Windows driver, a hypervisor, a security product, a forensic tool, or a research project, the same primitives should remain applicable with minimal assumptions about the surrounding runtime Whenever practical, API are written to operate correctly even in constrained execution contexts, making a large portion of the library usable at elevated IRQLs Components that inherently depend on facilities such as dynamic memory allocation or hook management naturally retain those requirements

---

## Highlights

- 🦀 Safe RAII wrappers around Windows kernel allocations
- 📦 Global allocator backed by the Windows kernel pool
- 🧠 Generic x86-64 page table walker
- 📄 Safe abstractions for Memory Descriptor Lists (MDL)
- 🔍 Kernel module and export resolution
- 🧩 `EPROCESS` utilities and process traversal helpers
- ✍️ Virtual memory read/write helpers
- 🪝 Lightweight kernel inline hooking with automatic trampolines
- ⚡ Zero-cost abstractions whenever possible
- 🔧 Raw `ntddk`, `ntifs` and `ntstatus` bindings included

---

## Why winkernl-rs?

The main goal of winkernl-rs is to reduce the amount of repetitive boilerplate usually involved in Windows kernel development while staying as close as possible to the native NT API

Instead of trying to completely abstract away the Windows kernel, the library provides higher-level building blocks for common kernel tasks while keeping the concepts, flexibility, and transparency that developers already know

The idea is to combine the low-level control of NT interfaces with some of the benefits Rust provides, such as stronger type safety and safer resource management through its ownership model

---

## Table of Contents

- [Installation](#installation)
- [Features](#features)
- [Modules](#modules)
   - [`sys`](#sys)
   - [`kalloc`](#kalloc)
   - [`memory`](#memory)
   - [`process`](#process)
   - [`khook`](#khook)
   - [`winimpl`](#winimpl)
- [Design Philosophy](#design-philosophy)
- [Safety](#support-the-project-)
- [License](#license)

---


## Documentation

This README provides an overview of the library and its main components. For complete API documentation, including every public type, function, trait, and module, generate the documentation locally with `rustdoc`:

```bash
cargo doc --package winkernl-rs --no-deps --open
```

The generated documentation is available under `target/doc/`.

---


## Installation

`winkernl-rs` is currently distributed directly from GitHub

### Latest version

```toml
[dependencies]
winkernl-rs = { git = "https://githubcom/0xryka/winkernl-rs" }
```

### Specific release

```toml
[dependencies]
winkernl-rs = { git = "https://githubcom/0xryka/winkernl-rs", tag = "0.1.0" }
```

### Specific commit

```toml
[dependencies]
winkernl-rs = { git = "https://githubcom/0xryka/winkernl-rs", rev = "<commit-hash>" }
```

---

# Features

`winkernl-rs` supports multiple strategies for locating and linking against
`ntoskrnl.lib`

```toml
[features]
default = ["bundled-wdk"]

bundled-wdk = []
wdk-auto    = []
```

Only **one** strategy should be enabled at a time

---

## `bundled-wdk` *(default)*

The crate ships with its own copy of **`ntoskrnl.lib`**, meaning no Windows
Driver Kit installation is required

This is the recommended configuration for most users

```toml
[dependencies]
winkernl-rs = { git = "https://githubcom/0xryka/winkernl-rs" }
```

or explicitly:

```toml
[dependencies]
winkernl-rs = {
    git = "https://githubcom/0xryka/winkernl-rs",
    default-features = false,
    features = ["bundled-wdk"]
}
```

---

## `wdk-auto`

Instead of using the bundled library, `winkernl-rs` will automatically try to
locate **`ntoskrnl.lib`** from your local Windows Driver Kit

Search order:

1 Use the path stored inside `WINDOWS_KITS_KM_LIB` if the environment variable exists
2 Otherwise, automatically search for a compatible Windows Kit installation

Enable it with:

```toml
[dependencies]
winkernl-rs = {
    git = "https://githubcom/0xryka/winkernl-rs",
    default-features = false,
    features = ["wdk-auto"]
}
```

### Manual WDK path

If your WDK is installed in a custom location, set the environment variable

#### PowerShell

```powershell
$env:WINDOWS_KITS_KM_LIB="C:\Program Files (x86)\Windows Kits\10\Lib\10.0.22621.0\km\x64"
```

#### Command Prompt

```cmd
set WINDOWS_KITS_KM_LIB=C:\Program Files (x86)\Windows Kits\10\Lib\10.0.22621.0\km\x64
```

---

# Modules

The crate is split into several independent modules

```
winkernl-rs
├── sys
├── kalloc
├── memory
│   ├── allocator
│   ├── module
│   ├── mdl
│   ├── pagewalk
│   └── rw
├── process
├── khook
└── winimpl
```

---

# `sys`

Raw Windows kernel bindings generated from the WDK

The API intentionally stays as close as possible to the native C declarations

Contains bindings for:

- `ntddk`
- `ntifs`
- `ntstatus`

### Example

```rust
use winkrnl_rs::sys::*;

unsafe {
    ExFreePool(ptr);
}
```

---

# `kalloc`

High-level Rust allocation wrappers

Unlike the raw allocation routines exposed by `sys`, every allocation follows
Rust ownership semantics and is automatically released when dropped

Supports:

- Pool allocations
- Pool slices
- Physically contiguous allocations
- Executable pool allocations

## Pool allocation

```rust
use winkrnl_rs::kalloc::pool::PoolMemory;

let value = PoolMemory::new(42u32).unwrap();
```

```rust
let string = PoolMemory::new(String::from("hello")).unwrap();
```

---

## Pool slices

```rust
use winkrnl_rs::kalloc::pool::PoolMemorySlice;

let buffer = PoolMemorySlice::new(0u8, 4096).unwrap();
```

```rust
let buffer = unsafe { PoolMemorySlice::<u64>::new_zeroed(128).unwrap() };
```

---

## Contiguous memory

```rust
use winkrnl_rs::kalloc::contiguous::ContiguousMemory;

let page = ContiguousMemory::<[u8; PAGE_SIZE as usize]>::new(0).unwrap();
```

```rust
let object = ContiguousMemory::<u64>::new(123).unwrap();
```

---

# `memory`

Utilities related to memory management

```
memory
├── allocator
├── module
├── mdl
├── pagewalk
└── rw
```

---

## `memory::allocator`

Primitive allocation helpers

These are lightweight wrappers around the native Windows allocation API and
are mostly used internally by `kalloc`

Functions include:

- `alloc_pool`
- `free_pool`
- `alloc_contiguous_memory`
- `free_contiguous_memory`

### Example

```rust
let ptr = alloc_pool(POOL_TYPE::NonPagedPool, 128).unwrap();
```

```rust
free_pool(ptr.cast());
```

---

## `memory::module`

Kernel module helpers

Provides utilities for locating loaded kernel modules and resolving exported
symbols

### Example

```rust
let nt = get_system_module_base_address("ntoskrnlexe")?;
```

```rust
let ex_alloc = resolve_system_routine("ntoskrnlexe", "ExAllocatePool")?;
```

---

## `memory::mdl`

Safe wrapper around Memory Descriptor Lists (MDL)

Automatically unlocks, unmaps and frees the MDL through RAII

### Example

```rust
let mut mdl = Mdl::new(address, size).unwrap();

mdl.lock();
```

```rust
let mapping = mdlmap_locked::<u8>();

mapping.protect(PAGE_READWRITE);
```

---

## `memory::rw`

Convenience helpers for reading and writing memory

Supports:

- read-only kernel patching
- cross-process reads
- cross-process writes

### Example

```rust
write_to_read_only_memory(target, patch);
```

```rust
mm_read_virtual_memory_from_pid(
    pid,
    address,
    &mut buffer
)?;
```

---

## `memory::pagewalk`

Generic x86-64 page table walker

The walker is backend-independent and only requires a physical memory provider
implementing the `MemPhysical` trait

Features include:

- virtual → physical translation
- page table walking
- PML4/PDPT/PD/PT lookup
- PTE address retrieval
- arbitrary physical memory reads

### Example

```rust
let pa = virtual_to_physical(virtual_address, read_callback)?;
```

```rust
let pte = get_address_of_level(cr3, virtual_address, PageLevel::Pt, read_physical_callback)?;
```

---

## Pattern scanning

The `memory` module also includes a wildcard signature scanner

### Example

```rust
let pattern = [Some(0x48), Some(0x8B), None, None, Some(0xE8)]; // 48 8B ?? ?? E8
let result = pattern_search(base, image_size, &pattern);
```

---

# `process`

Helpers around Windows `EPROCESS`

Instead of relying exclusively on exported NT API, this module can traverse
kernel process structures directly

Features:

- locate `PsInitialSystemProcess`
- enumerate processes
- lookup by PID
- discover `EPROCESS` offsets
- access selected `EPROCESS` fields

### Example

```rust
let process = get_process_by_id(4)?;
```

```rust
let system = get_initial_system_process()?;
```

---

# `khook`

Runtime inline kernel hooks

Creates executable trampolines automatically while preserving the original
instructions

Supports:

- `jmp rel32`
- absolute jumps (`mov rax, imm64; jmp rax`)

### Example

```rust
let mut hook = KHook::new(src, dst, false)?;
hook.enable()?;
```

```rust
let original = hook.original::<FnType>();
original();
```

---

# `winimpl`

Platform-specific implementations built on top of the generic abstractions
provided by `winkernl-rs`

The first implementation currently provided is the Windows page-walking backend

It implements the `MemPhysical` trait using `MmCopyMemory`, allowing the
generic page walker (`memory::pagewalk`) to operate on the current system by
reading physical memory through the native Windows kernel API

This module is mainly intended as a ready-to-use implementation of the generic
page walker, while still allowing users to provide their own `MemPhysical`
backend if needed (DMA, hypervisor, custom driver, etc)

### Translate a virtual address

```rust
use winkrnl_rs::memory::pagewalk::winimpl::SysPhysReader;

let mut reader = SysPhysReader::new();

let physical = reader.virtual_to_physical(virtual_address)?;
```

### Retrieve the address of a paging structure

```rust
use winkrnl_rs::memory::pagewalk::PageLevel;
use winkrnl_rs::memory::pagewalk::winimpl::SysPhysReader;

let mut reader = SysPhysReader::new();

let pte_addr = reader.get_physical_address_of_level(virtual_address, PageLevel::Pt)?;
```

The module also exposes standalone helpers when creating a reader object is
not necessary

```rust
use winkrnl_rs::memory::pagewalk::winimpl::{
    read_physical_sys,
    translate_va_to_pa_sys,
};

let physical = translate_va_to_pa_sys(virtual_address)?;
```
---

# Global allocator

When the `alloc` crate is available, `winkernl-rs` also provides a global kernel
allocator backed by the Windows pool allocator

### Example

```rust
use winkernl_rs::kalloc::KernelAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;
```

```rust
let string = String::from("winkernl-rs");
```

---

# Design philosophy

`winkernl-rs` does **not** attempt to hide the Windows kernel programming model

Instead, it focuses on:

- providing safe and idiomatic Rust abstractions over common kernel primitives;
- leveraging RAII to automatically manage kernel resources;
- reducing repetitive boilerplate without sacrificing flexibility;
- exposing reusable, generic components (such as the page walker) that can be adapted to different backends;
- remaining as close as possible to the native NT API and programming model;
- introducing little to no runtime overhead compared to equivalent C implementations

The goal of the library is to make Windows kernel development more ergonomic in
Rust while preserving the explicit control expected in Ring 0 programming



# Support the project ⭐

If `winkernl-rs` helped you, taught you something, or saved you from writing a few hundred lines of WDK boilerplate, consider giving the repository a ⭐!

It only takes a second, but it helps the project gain visibility and motivates me to continue improving it

I'm also working on additional low-level Rust libraries for Windows (and probably a few other systems as well), so more projects will be published in the future

`winkernl-rs` is currently available through github, but a release on **crates.io** is planned soon to make installation even easier


# License

This project is licensed under the **MIT License**

You are free to use, modify, distribute, and integrate it into both open-source and proprietary projects, provided that the original license is preserved

See the [`LICENSE`](LICENSE) file for the full license text