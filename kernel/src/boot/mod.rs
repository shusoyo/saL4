#![allow(dead_code)]

mod mm;
mod rootserver;

use crate::{
    cap::{CNodeCap, Capability, FrameCap, IRQControlCap, Slot, TCBCap, UntypedCap, threads::Tcb},
    config,
};
use mm::{BootArena, UntypedStream, probe_free_memory};

use core::ptr::NonNull;
use rootserver::{
    RootserverImage, enter_rootserver, locate_rootserver, prepare_rootserver_context,
};
use sal4_common::{BootInfo, CPtr, SlotRegion};
use tg_console::log;

const _: () = {
    assert!(config::ROOT_CNODE_SLOTS == (1usize << config::ROOT_CNODE_RADIX_BITS));
    assert!(config::ROOT_CNODE_SLOT_NULL < config::ROOT_CNODE_SLOTS);
    assert!(config::ROOT_CNODE_SLOT_TCB < config::ROOT_CNODE_SLOTS);
    assert!(config::ROOT_CNODE_SLOT_CNODE < config::ROOT_CNODE_SLOTS);
    assert!(config::ROOT_CNODE_SLOT_IRQ_CTRL < config::ROOT_CNODE_SLOTS);
    assert!(config::ROOT_CNODE_SLOT_BOOTINFO_FRAME < config::ROOT_CNODE_SLOTS);
    assert!(config::ROOT_CNODE_SLOT_IPC_BUFFER < config::ROOT_CNODE_SLOTS);
    assert!(config::FIRST_USER_IMAGE_FRAME_SLOT <= config::ROOT_CNODE_SLOTS);
};

/*
1. 物理内存探测：找到所有的空闲内存块（Free Memory）。

2. 内核预留：
  - 从 Free Memory 中切出一块给 BootInfo。
  - 从 Free Memory 中切出一块给 Root Task 的 TCB。
  - (TODO) 从 Free Memory 中切出一块给 Root Task 的 VSpace (顶级页表)。
  - 从 Free Memory 中切出一块给 Root Task 的 CNode。

3. 能力映射：
  - 把剩余的所有 Free Memory 封装成 Untyped 条目。
  - 把这些条目通通写进 CNode 空间。
  - 把这些条目的物理地址和大小写进 BootInfo。
*/
pub fn bootstrap() -> ! {
    let rootserver = locate_rootserver();
    let (free_memory_start, free_memory_end) = probe_free_memory(rootserver);
    log::info!(
        "[boot] rootserver image: start={:#x}, end={:#x}, size={:#x}",
        rootserver.start,
        rootserver.end,
        rootserver.size(),
    );
    log::info!(
        "[boot] free memory: start={:#x}, end={:#x}, size={:#x}",
        free_memory_start,
        free_memory_end,
        free_memory_end.saturating_sub(free_memory_start)
    );

    let mut arena = BootArena::new(free_memory_start, free_memory_end);

    let bootinfo_ptr = arena.alloc_typed::<BootInfo>();
    let root_cnode_ptr = arena.alloc_slots(config::ROOT_CNODE_SLOTS);
    let root_tcb_ptr = arena.alloc_typed::<Tcb>();
    let ipc_buffer_paddr = arena.alloc_page_paddr();
    let rootserver_stack_paddr = arena.alloc_page_paddr();

    log::info!(
        "[boot] allocated boot objects: bootinfo={:#x}, cnode={:#x} (slots={}), tcb={:#x}, ipc={:#x}, rootserver_stack={:#x}",
        bootinfo_ptr.as_ptr() as usize,
        root_cnode_ptr.as_ptr() as usize,
        config::ROOT_CNODE_SLOTS,
        root_tcb_ptr.as_ptr() as usize,
        ipc_buffer_paddr,
        rootserver_stack_paddr,
    );

    let bootinfo = unsafe { &mut *bootinfo_ptr.as_ptr() };
    let root_tcb = unsafe { &mut *root_tcb_ptr.as_ptr() };
    let root_cnode = unsafe {
        core::slice::from_raw_parts_mut(root_cnode_ptr.as_ptr(), config::ROOT_CNODE_SLOTS)
    };

    install_initial_caps(
        root_cnode,
        root_tcb,
        bootinfo_ptr.as_ptr() as usize,
        ipc_buffer_paddr,
    );

    let untyped_start = BootArena::align_up(arena.cursor(), config::PAGE_SIZE);
    let first_untyped_slot = populate_user_image_frame_caps(rootserver, root_cnode, bootinfo);
    populate_untyped_caps(
        untyped_start,
        free_memory_end,
        first_untyped_slot,
        root_cnode,
        bootinfo,
    );

    bootinfo.ipc_buffer = ipc_buffer_paddr;
    bootinfo.init_thread_cnode_size_bits = config::ROOT_CNODE_RADIX_BITS;

    prepare_rootserver_context(
        root_tcb,
        rootserver,
        rootserver_stack_paddr,
        bootinfo_ptr.as_ptr() as usize,
        ipc_buffer_paddr,
    );

    enter_rootserver(root_tcb)
}

fn install_initial_caps(
    cnode: &mut [Slot],
    tcb: &mut Tcb,
    bootinfo_paddr: usize,
    ipc_paddr: usize,
) {
    let cnode_storage = NonNull::from(&mut *cnode).cast::<Slot>();
    let root_cnode_cap = Capability::CNode(CNodeCap {
        storage: cnode_storage,
        radix_bits: config::ROOT_CNODE_RADIX_BITS,
        guard_size: usize::BITS as u8 - config::ROOT_CNODE_RADIX_BITS,
        guard: 0,
    });

    // Materialize the root TCB object before publishing a capability to it.
    *tcb = Tcb {
        cspace_root: root_cnode_cap,
        ..Tcb::empty()
    };

    cnode[config::ROOT_CNODE_SLOT_CNODE] = Slot {
        cap: root_cnode_cap,
        mdb: crate::cap::MDBNode::empty(),
    };

    cnode[config::ROOT_CNODE_SLOT_TCB] = Slot {
        cap: Capability::Tcb(TCBCap {
            ptr: NonNull::from(&mut *tcb),
        }),
        mdb: crate::cap::MDBNode::empty(),
    };

    cnode[config::ROOT_CNODE_SLOT_IRQ_CTRL] = Slot {
        cap: Capability::IRQControl(IRQControlCap),
        mdb: crate::cap::MDBNode::empty(),
    };

    cnode[config::ROOT_CNODE_SLOT_BOOTINFO_FRAME] = Slot {
        cap: Capability::Frame(FrameCap {
            paddr: bootinfo_paddr,
            size_bits: config::MIN_UNTYPED_SIZE_BITS,
        }),
        mdb: crate::cap::MDBNode::empty(),
    };

    cnode[config::ROOT_CNODE_SLOT_IPC_BUFFER] = Slot {
        cap: Capability::Frame(FrameCap {
            paddr: ipc_paddr,
            size_bits: config::MIN_UNTYPED_SIZE_BITS,
        }),
        mdb: crate::cap::MDBNode::empty(),
    };

    // Fixed boot-time contract:
    // - BOOTINFO_FRAME points at the BootInfo page itself.
    // - IPC_BUFFER points at rootserver's initial IPC buffer page.
    // - user_image_frames starts after these fixed slots and covers only the
    //   loaded rootserver image pages.
}

fn populate_user_image_frame_caps(
    image: RootserverImage,
    cnode: &mut [Slot],
    bootinfo: &mut BootInfo,
) -> usize {
    let start = config::FIRST_USER_IMAGE_FRAME_SLOT;
    let image_start = image.start & !(config::PAGE_SIZE - 1);
    let image_end = BootArena::align_up(image.end, config::PAGE_SIZE);
    let frame_count = image_end.saturating_sub(image_start) / config::PAGE_SIZE;
    let end = start + frame_count;

    assert!(
        end <= config::ROOT_CNODE_SLOTS,
        "rootserver image frame caps exceed root cnode capacity"
    );

    for (i, paddr) in (image_start..image_end)
        .step_by(config::PAGE_SIZE)
        .enumerate()
    {
        cnode[start + i] = Slot {
            cap: Capability::Frame(FrameCap {
                paddr,
                size_bits: config::MIN_UNTYPED_SIZE_BITS,
            }),
            mdb: crate::cap::MDBNode::empty(),
        };
    }

    bootinfo.user_image_frames = SlotRegion {
        start: CPtr::new(start),
        end: CPtr::new(end),
    };

    end
}

fn populate_untyped_caps(
    untyped_start: usize,
    free_memory_end: usize,
    first_untyped_slot: usize,
    cnode: &mut [Slot],
    bootinfo: &mut BootInfo,
) {
    let max_untyped = config::MAX_UNTYPED_OBJECTS
        .min(config::ROOT_CNODE_SLOTS.saturating_sub(first_untyped_slot));

    let stream = UntypedStream {
        current_paddr: untyped_start,
        end_paddr: free_memory_end,
    };

    let mut count = 0usize;
    for (i, (paddr, size_bits)) in stream.take(max_untyped).enumerate() {
        log::info!(
            "[boot] alloctated untyped region, phys addr: [{:#X}, {:#X}), size bits: {size_bits}",
            paddr,
            paddr + (1 << size_bits)
        );
        let slot_idx = first_untyped_slot + i;

        cnode[slot_idx] = Slot {
            cap: Capability::Untyped(UntypedCap {
                paddr,
                size_bits,
                is_device: false,
            }),
            mdb: crate::cap::MDBNode::empty(),
        };

        bootinfo.untyped_paddr_list[i] = paddr;
        bootinfo.untyped_size_bits_list[i] = size_bits;
        count += 1;
    }

    bootinfo.empty = SlotRegion {
        start: CPtr::new(first_untyped_slot + count),
        end: CPtr::new(config::ROOT_CNODE_SLOTS),
    };
    bootinfo.untyped = SlotRegion {
        start: CPtr::new(first_untyped_slot),
        end: CPtr::new(first_untyped_slot + count),
    };

    log::info!(
        "[boot] bootinfo regions: user_image=[{}, {}), untyped=[{}, {}), empty=[{}, {})",
        config::FIRST_USER_IMAGE_FRAME_SLOT,
        first_untyped_slot,
        first_untyped_slot,
        first_untyped_slot + count,
        first_untyped_slot + count,
        config::ROOT_CNODE_SLOTS,
    );
}

/*
Physical Address Space (RAM)                     Detailed Free Memory Layout (BootArena)
      +-----------------------+ <--- RAM_END ---+-----------------------+ <--- free_memory_end
      |                       |        |        |  Untyped Block (2^n)  |
      |      Free Memory      |        |        |-----------------------|
      |   (Detailed Right)    |--------+        |          ...          |
      |                       |        |        |-----------------------| <--- untyped_start
      |                       |        |        |/////// Padding ///////|
      +=======================+ <--- __end -----+-----------------------+
      |   Boot Stack (64KB)   |        ^        |   IPC Buffer (Page)   |
      +-----------------------+        |        +-----------------------+
      |    .bss (Zero-init)   |        |        |     Root Task TCB     |
      +-----------------------+        |        +-----------------------+
      |    .data / .rodata    |        |        |    Root CNode Slots   |
      +-----------------------+        |        +-----------------------+
      |   .text (Kernel)      |        |        |   BootInfo Structure  |
      +-----------------------+        +--------+-----------------------+ <--- free_memory_start
      |          ...          |                 (Allocated sequentially)
      +-----------------------+
      |   M-Mode Base / Stack |
      +-----------------------+ <--- 0x0


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
