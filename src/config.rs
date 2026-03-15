pub const PAGE_SIZE: usize = 4096;

pub const ROOT_CNODE_RADIX_BITS: u8 = 8;
pub const ROOT_CNODE_SLOTS: usize = 1usize << ROOT_CNODE_RADIX_BITS;
pub const MAX_UNTYPED_OBJECTS: usize = 128;

pub const ROOT_CNODE_SLOT: usize = 1;
pub const ROOT_TCB_SLOT: usize = 2;
pub const FIRST_UNTYPED_SLOT: usize = 4;

pub const MEMORY_SIAE: usize = 1 << 23;
