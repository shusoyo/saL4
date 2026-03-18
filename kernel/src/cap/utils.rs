#![allow(dead_code)]

use core::ptr::NonNull;

use crate::cap::{CNodeCap, Capability, Slot};
pub use sal4_common::CPtr;

#[derive(Debug)]
pub struct UncheckedSlot(NonNull<Slot>);

#[derive(Debug)]
pub struct PresentSlot(NonNull<Slot>);

#[derive(Debug)]
pub struct EmptySlot(NonNull<Slot>);

impl UncheckedSlot {
    pub fn ensure_present(self) -> Result<PresentSlot, LookupError> {
        ensure_present_slot(self.0)?;
        Ok(PresentSlot(self.0))
    }

    pub fn ensure_empty(self) -> Result<EmptySlot, LookupError> {
        ensure_empty_slot(self.0)?;
        Ok(EmptySlot(self.0))
    }
}

impl PresentSlot {
    pub fn cap(&self) -> Capability {
        unsafe { self.0.as_ref().cap }
    }

    pub fn cap_mut(&mut self) -> &mut Capability {
        unsafe { &mut self.0.as_mut().cap }
    }

    pub fn snapshot(&self) -> Slot {
        unsafe { *self.0.as_ref() }
    }
}

impl EmptySlot {
    pub fn write(self, slot: Slot) {
        unsafe {
            self.0.as_ptr().write(slot);
        }
    }
}

fn resolve_address_bits(
    root: &CNodeCap,
    cptr: CPtr,
    mut bits_left: u32,
) -> Result<NonNull<Slot>, LookupError> {
    let raw = cptr.raw();
    let mut current = *root;

    let mut extract_bits = |match_size: u32| {
        if bits_left < match_size {
            return Err(LookupError::InvalidPath);
        }

        bits_left -= match_size;
        let ext_val = (raw >> bits_left) & ((1 << match_size) - 1);
        Ok(ext_val)
    };

    loop {
        let flag = extract_bits(current.guard_size as u32)?;
        if flag as u64 != current.guard as u64 {
            return Err(LookupError::GuardMismatch);
        }

        let index = extract_bits(current.radix_bits as u32)?;

        let target_ptr = unsafe { current.storage.as_ptr().add(index) };
        let target = unsafe { target_ptr.read() };

        match target.cap {
            Capability::CNode(next) => {
                current = next;
            }
            _ if bits_left == 0 => return Ok(unsafe { NonNull::new_unchecked(target_ptr) }),
            _ => return Err(LookupError::TooDeep),
        }
    }
}

pub fn lookup_slot(
    root: &CNodeCap,
    cptr: CPtr,
    bits_left: u32,
) -> Result<UncheckedSlot, LookupError> {
    resolve_address_bits(root, cptr, bits_left).map(UncheckedSlot)
}

pub fn lookup_source_slot(
    root: &CNodeCap,
    cptr: CPtr,
    bits_left: u32,
) -> Result<PresentSlot, LookupError> {
    lookup_slot(root, cptr, bits_left)?.ensure_present()
}

pub fn lookup_target_slot(
    root: &CNodeCap,
    cptr: CPtr,
    bits_left: u32,
) -> Result<UncheckedSlot, LookupError> {
    lookup_slot(root, cptr, bits_left)
}

pub fn lookup_empty_slot(
    root: &CNodeCap,
    cptr: CPtr,
    bits_left: u32,
) -> Result<EmptySlot, LookupError> {
    lookup_slot(root, cptr, bits_left)?.ensure_empty()
}

pub fn lookup_cap_and_slot(
    root: &CNodeCap,
    cptr: CPtr,
    bits_left: u32,
) -> Result<(Capability, PresentSlot), LookupError> {
    let slot = lookup_source_slot(root, cptr, bits_left)?;
    let cap = slot.cap();
    Ok((cap, slot))
}

pub fn ensure_present_slot(slot: NonNull<Slot>) -> Result<(), LookupError> {
    let slot = unsafe { slot.as_ref() };
    if matches!(slot.cap, Capability::Null) {
        Err(LookupError::EmptySlot)
    } else {
        Ok(())
    }
}

pub fn ensure_empty_slot(slot: NonNull<Slot>) -> Result<(), LookupError> {
    let slot = unsafe { slot.as_ref() };
    if matches!(slot.cap, Capability::Null) {
        Ok(())
    } else {
        Err(LookupError::SlotNotEmpty)
    }
}

pub fn resolve_cptr(root: &CNodeCap, cptr: CPtr, bits_left: u32) -> Result<Slot, LookupError> {
    let (_, slot) = lookup_cap_and_slot(root, cptr, bits_left)?;
    Ok(slot.snapshot())
}

#[derive(Debug, Eq, PartialEq)]
pub enum LookupError {
    /// Guard 校验失败：CPtr 的高位与 CNode 的 guard 不匹配
    GuardMismatch,
    /// 路径过深：在 CPtr 位数未耗尽时遇到了非容器对象（如 TCB/Untyped）
    TooDeep,
    /// 路径不足：CPtr 位数耗尽，但尚未到达终端 Slot（例如停留在了中途的 CNode 上）
    InvalidPath,
    /// 槽位为空：目标 Slot 中的 Capability 是 Null
    EmptySlot,
    /// 目标槽位非空：期望写入空槽位时发现已有 Capability
    SlotNotEmpty,
}

pub fn align_up(x: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align must be 2^n");
    x.checked_add(align - 1).expect("overflow") & !(align - 1)
}
