pub const PAGE_SIZE: usize = 4096;
pub const MIN_UNTYPED_SIZE_BITS: u8 = 12;

pub const ROOT_CNODE_RADIX_BITS: u8 = 8;
pub const ROOT_CNODE_SLOTS: usize = 1usize << ROOT_CNODE_RADIX_BITS;
pub const MAX_UNTYPED_OBJECTS: usize = 128;

pub const ROOT_CNODE_SLOT_NULL: usize = 0;
pub const ROOT_CNODE_SLOT_TCB: usize = 1;
pub const ROOT_CNODE_SLOT_CNODE: usize = 2;
pub const ROOT_CNODE_SLOT_IRQ_CTRL: usize = 3;
pub const ROOT_CNODE_SLOT_BOOTINFO_FRAME: usize = 4;
pub const ROOT_CNODE_SLOT_IPC_BUFFER: usize = 5;

pub const FIRST_USER_IMAGE_FRAME_SLOT: usize = ROOT_CNODE_SLOT_IPC_BUFFER + 1;
pub const ROOTSERVER_STACK_SIZE: usize = PAGE_SIZE;

/// Early-boot RAM budget used by the tutorial kernel on QEMU.
///
/// At this stage we intentionally do not parse a platform memory map yet, so
/// bootstrap treats `[kernel_end, kernel_end + EARLY_BOOT_MEMORY_SIZE)` as the
/// boot-time free memory window.
pub const EARLY_BOOT_MEMORY_SIZE: usize = 1 << 23;
