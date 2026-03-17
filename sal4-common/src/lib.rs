#![no_std]

pub mod sal4_syscall;

pub const MAX_UNTYPED_OBJECTS: usize = 128;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CPtr(usize);

impl CPtr {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> usize {
        self.0
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SlotRegion {
    /// Inclusive start slot index in the root cnode.
    pub start: CPtr,
    /// Exclusive end slot index in the root cnode.
    pub end: CPtr,
}

#[repr(C, align(4096))]
#[derive(Debug, Copy, Clone)]
pub struct BootInfo {
    /// Initial rootserver IPC buffer address.
    ///
    /// In the current single-address-space bootstrap model this is the direct
    /// address of the IPC buffer page. Once VSpace exists, this should become
    /// the user-visible IPC buffer virtual address instead.
    pub ipc_buffer: usize,
    /// Radix bits of the initial thread's root cnode.
    pub init_thread_cnode_size_bits: u8,

    /// Unused root cnode slots available after bootstrap.
    ///
    /// `empty.start` is the first slot not consumed by the fixed boot-time
    /// allocations below (`TCB`, `CNode`, `IRQ control`, `BootInfo` frame,
    /// IPC buffer frame, rootserver image frames, exported untyped caps).
    pub empty: SlotRegion,
    /// Root cnode slots describing the loaded rootserver image frames.
    ///
    /// This region covers only the boot-time rootserver image that the kernel
    /// copies to its fixed run address. It does not describe the BootInfo
    /// frame, IPC buffer frame, rootserver stack, or any future user images
    /// loaded by rootserver itself.
    pub user_image_frames: SlotRegion,
    /// Root cnode slots holding the exported untyped capabilities.
    ///
    /// This region starts after the fixed boot-time objects and excludes the
    /// rootserver image, BootInfo frame, IPC buffer frame, and rootserver
    /// stack reserved during bootstrap.
    pub untyped: SlotRegion,

    /// Physical base address of each exported untyped capability.
    pub untyped_paddr_list: [usize; MAX_UNTYPED_OBJECTS],
    /// Size bits of each exported untyped capability.
    pub untyped_size_bits_list: [u8; MAX_UNTYPED_OBJECTS],
}
