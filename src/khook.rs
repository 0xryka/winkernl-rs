//! Runtime kernel function hooking.
//!
//! This module implements lightweight inline hooks for x86-64 Windows
//! kernel code.
//!
//! A [`KHook`] redirects execution from a source function to a user-defined
//! destination while preserving the overwritten instructions inside an
//! executable trampoline.
//!
//! The generated trampoline contains:
//!
//! 1. The original instructions overwritten by the hook.
//! 2. A jump back to the instruction immediately following the hook.
//!
//! Calling [`KHook::original`] returns a function pointer to this trampoline,
//! allowing the original routine to continue execution transparently.
//!
//! # Jump generation
//!
//! Two jump encodings are supported:
//!
//! - **Relative jump (5 bytes)**
//!
//! ```text
//! E9 xx xx xx xx
//! ```
//!
//! Used whenever the destination lies within the ±2 GiB range.
//!
//! - **Absolute jump (13 bytes)**
//!
//! ```text
//! mov rax, <target>
//! jmp rax
//! ```
//!
//! Used when the destination cannot be reached with a relative jump.
//!
//! # Trampoline construction
//!
//! During hook creation the beginning of the target routine is decoded until
//! enough instructions have been collected to overwrite the largest possible
//! hook (`MAX_HOOK_SIZE`).
//!
//! Those instructions are copied into executable memory before appending a
//! jump returning execution to the original function.
//!
//! # RIP-relative instructions
//!
//! Relocating instructions that reference memory through RIP-relative
//! addressing requires adjusting their displacement.
//!
//! This implementation intentionally does **not** rewrite such instructions.
//! By default, hook creation fails when one is encountered.
//!
//! Passing `ignore_rip_relative = true` disables this verification and simply
//! copies the instruction into the trampoline. Doing so is only safe if the
//! relocated instruction does not depend on its original instruction pointer.
//!
//! # Notes
//!
//! Installing or removing a hook modifies executable kernel memory through
//! the facilities provided by the `memory::rw` module.
//!
//! Hooks are automatically removed when the [`KHook`] instance is dropped.
use core::mem;
use iced_x86::*;
use x86_64::VirtAddr;
use crate::*;
use crate::kalloc::pool::PoolMemorySlice;
use crate::memory::rw::write_to_read_only_memory;


/// Represents an installed or installable inline kernel hook.
///
/// A hook owns an executable trampoline containing the original instructions
/// overwritten by the hook as well as a jump back into the original routine.
///
/// Dropping a hook automatically restores the original function bytes.
pub struct KHook {
    /// Address of the function being hooked.
    src: *mut u8,

    /// Hook destination.
    dst: *mut u8,

    /// Executable trampoline containing the relocated original code.
    original_stub: PoolMemorySlice<u8>,

    /// Original bytes overwritten in the target function.
    original_bytes: Vec<u8>,

    /// Whether the hook is currently installed.
    enabled: bool,
}

/// Size, in bytes, of a relative `jmp rel32` instruction.
pub const HOOK_SIZE_RELATIVE: usize = 5;
/// Maximum size, in bytes, of the absolute jump sequence.
///
/// This corresponds to:
///
/// ```text
/// mov rax, imm64
/// jmp rax
/// ```
pub const HOOK_SIZE_ABSOLUTE: usize = 13;
/// Largest hook sequence emitted by this module.
pub const MAX_HOOK_SIZE: usize = HOOK_SIZE_ABSOLUTE;
const MOV_ABS_SIZE: usize = 10;
const JMP_REG_MAX_SIZE: usize = 3;

/// Errors that can occur while constructing a hook.
#[derive(Debug)]
pub enum KhookErr {
    /// A RIP-relative instruction was encountered.
    ///
    /// Since relocating such instructions requires rewriting their
    /// displacement, hook creation is aborted unless explicitly allowed.
    InsnRel,

    /// The instruction decoder encountered an invalid instruction.
    InvalidInsn,

    /// Failed to allocate the executable trampoline.
    AllocationFailed,
}


impl KHook {
    // fn push_register(reg: Register) -> Instruction {
    //     let mut insn = Instruction::new();
    //     insn.set_code(Code::Push_r64);
    //     insn.set_op0_register(reg);
    //     insn
    // }

    fn create_mov_immediat(reg: Register, imm64: u64) -> [u8; MOV_ABS_SIZE] {
        let (rex, opcode) = match reg {
            Register::RAX => (0x48, 0xB8),
            Register::RCX => (0x48, 0xB9),
            Register::RDX => (0x48, 0xBA),
            Register::RBX => (0x48, 0xBB),
            Register::RSP => (0x48, 0xBC),
            Register::RBP => (0x48, 0xBD),
            Register::RSI => (0x48, 0xBE),
            Register::RDI => (0x48, 0xBF),

            Register::R8  => (0x49, 0xB8),
            Register::R9  => (0x49, 0xB9),
            Register::R10 => (0x49, 0xBA),
            Register::R11 => (0x49, 0xBB),
            Register::R12 => (0x49, 0xBC),
            Register::R13 => (0x49, 0xBD),
            Register::R14 => (0x49, 0xBE),
            Register::R15 => (0x49, 0xBF),
            _ => panic!("Register not supported"),
        };

        let mut bytes = [0u8; MOV_ABS_SIZE];
        bytes[0] = rex;
        bytes[1] = opcode;
        bytes[2..].copy_from_slice(&imm64.to_le_bytes());
        bytes
    }

    fn create_jmp_on_reg(reg: Register) -> ([u8; JMP_REG_MAX_SIZE], usize) {
        let (rex, rm) = match reg {
            Register::RAX => (None, 0),
            Register::RCX => (None, 1),
            Register::RDX => (None, 2),
            Register::RBX => (None, 3),
            Register::RSP => (None, 4),
            Register::RBP => (None, 5),
            Register::RSI => (None, 6),
            Register::RDI => (None, 7),

            Register::R8  => (Some(0x41), 0),
            Register::R9  => (Some(0x41), 1),
            Register::R10 => (Some(0x41), 2),
            Register::R11 => (Some(0x41), 3),
            Register::R12 => (Some(0x41), 4),
            Register::R13 => (Some(0x41), 5),
            Register::R14 => (Some(0x41), 6),
            Register::R15 => (Some(0x41), 7),
            _ => panic!("register not supported"),
        };

        let modrm = 0b11_100_000 | rm;

        let mut bytes = [0u8; JMP_REG_MAX_SIZE];
        if let Some(rex) = rex {
            bytes[0] = rex;
            bytes[1] = 0xFF;
            bytes[2] = modrm;
            (bytes, 3)
        } else {
            bytes[0] = 0xFF;
            bytes[1] = modrm;
            (bytes, 2)
        }
    }

    fn create_jmp_rel32(offset: i32) -> [u8; HOOK_SIZE_RELATIVE] {
        let mut bytes = [0u8; HOOK_SIZE_RELATIVE];
        bytes[0] = 0xE9;
        bytes[1..5].copy_from_slice(&offset.to_le_bytes());
        bytes
    }

    fn create_assembly_jump(stub_addr: u64, target_addr: u64) -> Vec<u8> {
        let rel = target_addr as i64 - (stub_addr as i64 + HOOK_SIZE_RELATIVE as i64);
        if let Ok(rel) = i32::try_from(rel) {
            Self::create_jmp_rel32(rel).to_vec()
        } else {
            let mut bytes = Vec::with_capacity(MOV_ABS_SIZE + JMP_REG_MAX_SIZE);
            bytes.extend_from_slice(&Self::create_mov_immediat(Register::RAX, target_addr));
            let (jmp, len) = Self::create_jmp_on_reg(Register::RAX);
            bytes.extend_from_slice(&jmp[..len]);
            bytes
        }
    }

    /// Creates a new hook without installing it.
    ///
    /// This allocates an executable trampoline containing the overwritten
    /// instructions followed by a jump back into the original function.
    ///
    /// If `ignore_rip_relative` is `false`, hook creation fails whenever a
    /// RIP-relative instruction is encountered.
    pub fn new(src: *mut u8, dst: *mut u8, ignore_rip_relative: bool) -> Result<Self, KhookErr> {
        let src_address = src as u64;

        let src_slice = unsafe {
            slice::from_raw_parts(src_address as *const u8, MAX_HOOK_SIZE + 16)
        };

        let mut decoder = Decoder::with_ip(64, src_slice, src_address, DecoderOptions::NONE);
        let mut copied = 0usize;

        while copied < MAX_HOOK_SIZE {
            let inst = decoder.decode();
            if inst.mnemonic() == Mnemonic::INVALID {
                return Err(KhookErr::InvalidInsn)
            }
            if inst.is_ip_rel_memory_operand() && !ignore_rip_relative {
                return Err(KhookErr::InsnRel);
            }
            copied += inst.len();
        }

        let original_bytes = src_slice[..copied].to_vec();
        let mut original_stub = PoolMemorySlice::new_with_pool_type(0u8, copied + HOOK_SIZE_ABSOLUTE, POOL_TYPE::NonPagedPoolExecute).ok_or(KhookErr::AllocationFailed)?;
        original_stub[..copied].copy_from_slice(&original_bytes);
        let jump_ip = original_stub.as_ptr() as u64 + copied as u64;
        let target_jump = src as u64 + copied as u64;
        let asm_jump = Self::create_assembly_jump(jump_ip, target_jump);
        original_stub[copied..copied+asm_jump.len()].copy_from_slice(asm_jump.as_slice());
        Ok(Self {
            src, dst, original_stub, original_bytes, enabled: false,
        })
    }

    /// Installs the hook.
    ///
    /// The beginning of the source routine is replaced with either a relative
    /// or absolute jump depending on the destination address.
    ///
    /// Calling this function on an already enabled hook has no effect.
    pub fn enable(&mut self) -> Option<()> {
        if self.enabled {
            return Some(())
        }
        let src = self.src as u64;
        let dst = self.dst as u64;
        let asm = Self::create_assembly_jump(src, dst);
        if !write_to_read_only_memory(VirtAddr::new(self.src as _), &asm) {
            return None;
        }
        self.enabled = true;
        Some(())
    }

    /// Returns whether the hook is currently installed.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }


    /// Restores the original function bytes.
    ///
    /// Calling this function on a disabled hook has no effect.
    pub fn disable(&mut self) -> Option<()> {
        if self.enabled {
            if !write_to_read_only_memory(VirtAddr::new(self.src as _), &self.original_bytes) {
                return None;
            }
        }
        self.enabled = false;
        Some(())
    }

    /// Returns a callable pointer to the trampoline containing the original
    /// implementation.
    ///
    /// # Panics
    ///
    /// Panics if `F` is not the size of a function pointer.
    pub fn original<F: Copy>(&self) -> F {
        assert_eq!(size_of::<F>(), 8, "F must be a function pointer type");
        unsafe { mem::transmute_copy(&self.original_stub.as_ptr()) }
    }
}



impl Drop for KHook {
    fn drop(&mut self) {
        self.disable();
    }
}
