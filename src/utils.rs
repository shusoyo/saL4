#![allow(dead_code)]

use crate::cap::{CNodeCap, Capability, Slot};

#[repr(transparent)]
#[derive(Debug, Eq, PartialEq)]
pub struct CPtr(usize);

impl CPtr {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }
}

pub fn resolve_cptr(root: &CNodeCap, cptr: CPtr, mut bits_left: u32) -> Result<Slot, LookupError> {
    let safed_minus = |bits_left: u32, match_size: u32| {
        if bits_left < match_size {
            Err(LookupError::InvalidPath)
        } else {
            Ok(bits_left - match_size)
        }
    };

    bits_left = safed_minus(bits_left, root.guard_size as u32)?;

    let flag = cptr.0 >> (bits_left) & ((1 << root.guard_size) - 1);

    if flag != root.guard {
        return Err(LookupError::GuardMismatch);
    }

    bits_left = safed_minus(bits_left, root.radix_bits as u32)?;

    let index = (cptr.0 >> bits_left) & ((1 << root.radix_bits) - 1);

    let target = unsafe { root.storage.as_ptr().add(index).read() };

    match target.cap {
        Capability::CNode(next) => resolve_cptr(&next, cptr, bits_left),
        Capability::Null => Err(LookupError::EmptySlot),
        _ if bits_left == 0 => Ok(target),
        _ => Err(LookupError::TooDeep),
    }
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
}
