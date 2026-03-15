#![allow(dead_code)]

use crate::{
    cap::{CNodeCap, Capability, Slot, TCBCap, UntypedCap, threads::Tcb},
    config,
    utils::CPtr,
};
use core::{
    mem::{align_of, size_of},
    ptr::NonNull,
};

struct EarlyAllocator {
    current: usize,
    end: usize,
}

impl EarlyAllocator {
    fn new(start: usize, end: usize) -> Self {
        Self {
            current: Self::align_up(start, config::PAGE_SIZE),
            end,
        }
    }

    fn align_up(x: usize, align: usize) -> usize {
        assert!(align != 0, "align must not be zero");
        let rem = x % align;
        if rem == 0 { x } else { x + (align - rem) }
    }

    fn alloc(&mut self, size: usize, align: usize) -> usize {
        assert!(align != 0, "align must not be zero");
        self.current = Self::align_up(self.current, align);
        let addr = self.current;
        self.current += size;
        assert!(self.current <= self.end, "Out of memory during bootstrap");
        addr
    }

    fn cursor(&self) -> usize {
        self.current
    }
}

struct ReservedLayout {
    bootinfo_ptr: usize,
    cnode_ptr: usize,
    cnode_slots: usize,
    tcb_ptr: usize,
    untyped_start_paddr: usize,
}

struct UntypedBootstrap {
    count: usize,
    paddr_list: [usize; config::MAX_UNTYPED_OBJECTS],
    size_bits_list: [u8; config::MAX_UNTYPED_OBJECTS],
}

#[repr(C)]
pub struct SlotRegion {
    // [start, end)
    pub start: CPtr,
    pub end: CPtr,
}

#[repr(C, align(4096))]
pub struct BootInfo {
    // pub node_id: usize,
    // pub num_nodes: usize,
    // pub ipc_buffer_vaddr: usize,
    pub init_thread_cnode_size_bits: u8,

    pub empty: SlotRegion,
    pub user_image_frames: SlotRegion,
    pub user_image_paging: SlotRegion,
    pub untyped: SlotRegion,

    pub untyped_paddr_list: [usize; config::MAX_UNTYPED_OBJECTS],
    pub untyped_size_bits_list: [u8; config::MAX_UNTYPED_OBJECTS],
    // pub fdt_paddr: usize,
    // pub fdt_size: usize,
}

pub struct BootManager;

impl BootManager {
    /*
    1. 物理内存探测：找到所有的空闲内存块（Free Memory）。

    2. 内核预留：
      - 从 Free Memory 中切出一块给 BootInfo。
      - 从 Free Memory 中切出一块给 Root Task 的 TCB。
      - 从 Free Memory 中切出一块给 Root Task 的 VSpace (顶级页表)。
      - 从 Free Memory 中切出一块给 Root Task 的 CNode（这是最重要的资源池）。

    3. 能力映射：
      - 把剩余的所有 Free Memory 封装成 Untyped 条目。
      - 把这些条目通通写进 CNode 空间。
      - 把这些条目的物理地址和大小写进 BootInfo。
    */
    pub fn bootstrap(&mut self) {
        let (free_memory_start, free_memory_end) = self.probe_free_memory();
        let reserved = self.reserve_boot_objects(free_memory_start, free_memory_end);

        let cnode_storage = self.init_root_cnode_storage(reserved.cnode_ptr, reserved.cnode_slots);
        self.install_initial_caps(cnode_storage, reserved.tcb_ptr);

        let untyped = self.populate_untyped_caps(
            cnode_storage,
            reserved.untyped_start_paddr,
            free_memory_end,
            reserved.cnode_slots,
        );

        self.write_bootinfo(reserved.bootinfo_ptr, reserved.cnode_slots, &untyped);
    }

    fn probe_free_memory(&self) -> (usize, usize) {
        // [__end, __end + MEMORY_SIAE) is treated as free memory for early boot.
        let locate = tg_linker::KernelLayout::locate();
        let free_memory_start = locate.end();
        let free_memory_end = free_memory_start + config::MEMORY_SIAE;
        (free_memory_start, free_memory_end)
    }

    fn reserve_boot_objects(
        &self,
        free_memory_start: usize,
        free_memory_end: usize,
    ) -> ReservedLayout {
        let mut allocator = EarlyAllocator::new(free_memory_start, free_memory_end);

        let bootinfo_ptr = allocator.alloc(size_of::<BootInfo>(), config::PAGE_SIZE);

        let cnode_slots = config::ROOT_CNODE_SLOTS;
        let cnode_size = cnode_slots * size_of::<Slot>();
        // CNode storage is kernel-private metadata, so natural type alignment is enough.
        let cnode_ptr = allocator.alloc(cnode_size, align_of::<Slot>());

        // TCB needs stable field offsets (repr(C)), but not page alignment here.
        let tcb_ptr = allocator.alloc(size_of::<Tcb>(), align_of::<Tcb>());

        ReservedLayout {
            bootinfo_ptr,
            cnode_ptr,
            cnode_slots,
            tcb_ptr,
            untyped_start_paddr: EarlyAllocator::align_up(allocator.cursor(), config::PAGE_SIZE),
        }
    }

    fn init_root_cnode_storage(&self, cnode_ptr: usize, cnode_slots: usize) -> NonNull<Slot> {
        let cnode_storage = cnode_ptr as *mut Slot;
        for i in 0..cnode_slots {
            unsafe { cnode_storage.add(i).write(Slot::empty()) };
        }
        NonNull::new(cnode_storage).expect("cnode storage pointer must not be null")
    }

    fn install_initial_caps(&self, cnode_storage: NonNull<Slot>, tcb_ptr: usize) {
        let root_cnode_cap = Capability::CNode(CNodeCap {
            storage: cnode_storage,
            radix_bits: config::ROOT_CNODE_RADIX_BITS,
            guard_size: 0,
            guard: 0,
        });

        let tcb_nn = NonNull::new(tcb_ptr as *mut Tcb).expect("tcb pointer must not be null");

        // Materialize the root TCB object before publishing a capability to it.
        unsafe {
            tcb_nn.as_ptr().write(Tcb {
                cspace_root: root_cnode_cap,
                ..Tcb::empty()
            });
        }

        unsafe {
            cnode_storage
                .as_ptr()
                .add(config::ROOT_CNODE_SLOT)
                .write(Slot {
                    cap: root_cnode_cap,
                    mdb: crate::cap::MDBNode::empty(),
                });

            cnode_storage
                .as_ptr()
                .add(config::ROOT_TCB_SLOT)
                .write(Slot {
                    cap: Capability::Tcb(TCBCap { ptr: tcb_nn }),
                    mdb: crate::cap::MDBNode::empty(),
                });
        }
    }

    fn populate_untyped_caps(
        &self,
        cnode_storage: NonNull<Slot>,
        untyped_start: usize,
        free_memory_end: usize,
        cnode_slots: usize,
    ) -> UntypedBootstrap {
        let mut paddr_list = [0usize; config::MAX_UNTYPED_OBJECTS];
        let mut size_bits_list = [0u8; config::MAX_UNTYPED_OBJECTS];

        let mut untyped_count = 0usize;
        let mut untyped_paddr = untyped_start;
        let max_untyped_by_slots = cnode_slots.saturating_sub(config::FIRST_UNTYPED_SLOT);
        let max_untyped = core::cmp::min(config::MAX_UNTYPED_OBJECTS, max_untyped_by_slots);

        while untyped_paddr < free_memory_end && untyped_count < max_untyped {
            let remain = free_memory_end - untyped_paddr;
            if remain < config::PAGE_SIZE {
                break;
            }

            let (size_bits, block_size) = Self::pick_untyped_block(untyped_paddr, remain);

            paddr_list[untyped_count] = untyped_paddr;
            size_bits_list[untyped_count] = size_bits;

            unsafe {
                cnode_storage
                    .as_ptr()
                    .add(config::FIRST_UNTYPED_SLOT + untyped_count)
                    .write(Slot {
                        cap: Capability::Untyped(UntypedCap {
                            paddr: untyped_paddr,
                            size_bits,
                            is_device: false,
                        }),
                        mdb: crate::cap::MDBNode::empty(),
                    });
            }

            untyped_paddr += block_size;
            untyped_count += 1;
        }

        UntypedBootstrap {
            count: untyped_count,
            paddr_list,
            size_bits_list,
        }
    }

    fn pick_untyped_block(paddr: usize, remain: usize) -> (u8, usize) {
        // Keep untyped in 2^n sizes (seL4-like model), but use clear arithmetic checks
        // instead of bit tricks to make intent explicit.
        let mut size_bits = 12u8;
        for candidate in (12u8..=(usize::BITS - 1) as u8).rev() {
            let block_size = 1usize << candidate;
            if block_size <= remain && paddr.is_multiple_of(block_size) {
                size_bits = candidate;
                break;
            }
        }

        let block_size = 1usize << size_bits;

        (size_bits, block_size)
    }

    fn write_bootinfo(&self, bootinfo_ptr: usize, cnode_slots: usize, untyped: &UntypedBootstrap) {
        let bootinfo = BootInfo {
            init_thread_cnode_size_bits: config::ROOT_CNODE_RADIX_BITS,
            empty: SlotRegion {
                start: CPtr::new(config::FIRST_UNTYPED_SLOT + untyped.count),
                end: CPtr::new(cnode_slots),
            },
            // TODO: User image not populated yet.
            user_image_frames: SlotRegion {
                start: CPtr::new(0),
                end: CPtr::new(0),
            },
            user_image_paging: SlotRegion {
                start: CPtr::new(0),
                end: CPtr::new(0),
            },
            untyped: SlotRegion {
                start: CPtr::new(config::FIRST_UNTYPED_SLOT),
                end: CPtr::new(config::FIRST_UNTYPED_SLOT + untyped.count),
            },
            untyped_paddr_list: untyped.paddr_list,
            untyped_size_bits_list: untyped.size_bits_list,
        };

        unsafe {
            (bootinfo_ptr as *mut BootInfo).write(bootinfo);
        }
    }
    // fn bootstrap_untyped(&mut self);
    // fn craft_initial_caps(&mut self);
}

/*
Physical Address Space (RAM)
      +---------------------------------------+ <--- RAM_END (e.g., 0x8800_0000)
      |                                       |
      |        Free Memory                    |
      |   (EarlyAllocator starts here)        |
      |                                       |
      +=======================================+ <--- __end (Kernel Image End)
      |                                       |
      |      Boot Stack Space (64KB)          |
      |    [ Stack Grows Downwards \/ ]       |
      |                                       |
      +---------------------------------------+ <--- boot_stack_top (Initial SP)
      |                                       |
      |      .boot.stack buffer space         |
      |                                       |
      +=======================================+ <--- __boot / boot_stack (Start)
      |          .bss (Zero-init)             |
      +---------------------------------------+ <--- __ebss
      |          .data (Initialized)          |
      +---------------------------------------+ <--- __data
      |          .rodata (Read-only)          |
      +---------------------------------------+ <--- __rodata
      |          .text (Kernel Code)          |
      +---------------------------------------+ <--- S_BASE_ADDRESS (0x8020_0000)
      |                                       |
      |           Gap / Padding               |
      |                                       |
      +---------------------------------------+
      |          .bss.m_data / stack          |
      |---------------------------------------|
      |          .text.m_trap                 |
      |---------------------------------------|
      |          .text.m_entry (_m_start)     |
      +---------------------------------------+ <--- M_BASE_ADDRESS (0x8000_0000)
      |                                       |
      |           I/O & Platform ROM          |
      |                                       |
      +---------------------------------------+ <--- 0x0
*/
