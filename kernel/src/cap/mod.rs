#![allow(dead_code)]

pub mod invocation;
pub mod threads;
pub mod utils;

use core::ptr::NonNull;

#[derive(Debug, Copy, Clone)]
pub struct UntypedCap {
    pub paddr: usize,
    pub size_bits: u8,
    pub is_device: bool,
    pub free_offset: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct CNodeCap {
    pub storage: NonNull<Slot>,
    pub radix_bits: u8,
    pub guard_size: u8,
    pub guard: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct TCBCap {
    pub ptr: NonNull<threads::Tcb>,
}

#[derive(Debug, Copy, Clone)]
pub struct IRQControlCap;

#[derive(Debug, Copy, Clone)]
pub struct FrameCap {
    pub paddr: usize,
    pub size_bits: u8,
}

#[derive(Debug, Copy, Clone)]
pub enum Capability {
    Null,
    Untyped(UntypedCap),
    CNode(CNodeCap),
    Tcb(TCBCap),
    IRQControl(IRQControlCap),
    Frame(FrameCap),
}

#[derive(Debug, Copy, Clone)]
pub struct MDBNode {
    pub parent: Option<NonNull<Slot>>,
    pub next: Option<NonNull<Slot>>,
    pub prev: Option<NonNull<Slot>>,
}

impl MDBNode {
    pub const fn empty() -> Self {
        Self {
            parent: None,
            next: None,
            prev: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Slot {
    pub cap: Capability,
    pub mdb: MDBNode,
}

impl Slot {
    pub const fn empty() -> Self {
        Self {
            cap: Capability::Null,
            mdb: MDBNode::empty(),
        }
    }
}
