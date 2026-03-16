#![allow(dead_code)]

use crate::{
    cap::{CNodeCap, Capability, FrameCap, IRQControlCap, Slot, TCBCap, UntypedCap, threads::Tcb},
    config,
    utils::CPtr,
};
use core::{
    mem::{align_of, size_of},
    ptr::NonNull,
};
use riscv::register::{
    scause::{self, Exception, Trap},
    stval, stvec,
};
use tg_console::log;
use tg_kernel_context::LocalContext;
use tg_sbi::shutdown;

#[derive(Debug, Copy, Clone)]
pub struct RootserverImage {
    pub start: usize,
    pub end: usize,
}

impl RootserverImage {
    pub const fn size(&self) -> usize {
        self.end - self.start
    }
}

struct BootArena {
    current: usize,
    end: usize,
}

impl BootArena {
    fn new(start: usize, end: usize) -> Self {
        Self {
            current: Self::align_up(start, config::PAGE_SIZE),
            end,
        }
    }

    fn align_up(x: usize, align: usize) -> usize {
        assert!(align.is_power_of_two(), "align must be 2^n");
        x.checked_add(align - 1).expect("overflow") & !(align - 1)
    }

    fn alloc_typed<T>(&mut self) -> NonNull<T> {
        let align = align_of::<T>();

        let size = size_of::<T>();
        assert!(align != 0, "align must not be zero");

        self.current = Self::align_up(self.current, align);
        let ptr = self.current as *mut T;
        self.current += size;
        assert!(self.current <= self.end, "Out of memory during bootstrap");

        unsafe {
            // Boot-time kernel objects start from a deterministic zeroed state.
            core::ptr::write_bytes(ptr, 0, 1);
            NonNull::new_unchecked(ptr)
        }
    }

    fn alloc_slots(&mut self, count: usize) -> NonNull<Slot> {
        let align = align_of::<Slot>();
        assert!(align != 0, "align must not be zero");

        self.current = Self::align_up(self.current, align);
        let ptr = self.current as *mut Slot;

        self.current += size_of::<Slot>() * count;
        assert!(self.current <= self.end, "Out of memory during bootstrap");

        unsafe {
            core::ptr::write_bytes(ptr, 0, count);
            NonNull::new_unchecked(ptr)
        }
    }

    fn alloc_page_paddr(&mut self) -> usize {
        self.current = Self::align_up(self.current, config::PAGE_SIZE);
        let paddr = self.current;
        self.current += config::PAGE_SIZE;
        assert!(self.current <= self.end, "Out of memory during bootstrap");

        unsafe {
            core::ptr::write_bytes(paddr as *mut u8, 0, config::PAGE_SIZE);
        }

        paddr
    }

    fn cursor(&self) -> usize {
        self.current
    }
}

struct UntypedStream {
    current_paddr: usize,
    end_paddr: usize,
}

impl Iterator for UntypedStream {
    type Item = (usize, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let remain = self.end_paddr.saturating_sub(self.current_paddr);
        if remain < config::PAGE_SIZE {
            return None;
        }

        // Keep untyped in 2^n sizes (seL4-like model), but use clear arithmetic checks
        let s_paddr_bits = self.current_paddr.trailing_zeros();
        let remain_bits = remain.ilog2();
        let size_bits = s_paddr_bits.min(remain_bits);

        let paddr = self.current_paddr;
        self.current_paddr += 1usize << size_bits;
        Some((paddr, size_bits as u8))
    }
}

#[repr(C)]
pub struct SlotRegion {
    /// Inclusive start slot index in the root cnode.
    pub start: CPtr,
    /// Exclusive end slot index in the root cnode.
    pub end: CPtr,
}

#[repr(C, align(4096))]
pub struct BootInfo {
    // pub node_id: usize,
    // pub num_nodes: usize,
    // pub ipc_buffer_vaddr: usize,
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
    pub untyped_paddr_list: [usize; config::MAX_UNTYPED_OBJECTS],
    /// Size bits of each exported untyped capability.
    pub untyped_size_bits_list: [u8; config::MAX_UNTYPED_OBJECTS],
    // pub fdt_paddr: usize,
    // pub fdt_size: usize,
}

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

    bootinfo.init_thread_cnode_size_bits = config::ROOT_CNODE_RADIX_BITS;

    prepare_rootserver_context(root_tcb, rootserver, rootserver_stack_paddr);
    enter_rootserver(root_tcb)
}

pub fn locate_rootserver() -> RootserverImage {
    let image = tg_linker::AppMeta::locate()
        .iter()
        .next()
        .expect("rootserver image missing");
    RootserverImage {
        start: image.as_ptr() as usize,
        end: image.as_ptr() as usize + image.len(),
    }
}

fn probe_free_memory(rootserver: RootserverImage) -> (usize, usize) {
    // The rootserver image is already copied to its fixed run address by
    // AppMeta::iter(), so boot-time free memory starts after whichever is later:
    // the kernel image end or the rootserver image end.
    let locate = tg_linker::KernelLayout::locate();
    let kernel_end = locate.end();
    let free_memory_start = BootArena::align_up(kernel_end.max(rootserver.end), config::PAGE_SIZE);
    let free_memory_end = kernel_end + config::EARLY_BOOT_MEMORY_SIZE;
    (free_memory_start, free_memory_end)
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

fn prepare_rootserver_context(tcb: &mut Tcb, image: RootserverImage, stack_paddr: usize) {
    let mut ctx = LocalContext::user(image.start);
    *ctx.sp_mut() = stack_paddr + config::ROOTSERVER_STACK_SIZE;

    tcb.ctx = ctx;
    tcb.state = crate::cap::threads::ThreadState::Running;

    log::info!(
        "[boot] prepared rootserver context: pc={:#x}, sp={:#x}",
        tcb.ctx.pc(),
        tcb.ctx.sp()
    );
}

fn enter_rootserver(tcb: &mut Tcb) -> ! {
    log::info!("[boot] entering rootserver");
    let saved_stvec = stvec::read();
    let sstatus = unsafe { tcb.ctx.execute() };
    let saved_mode = saved_stvec
        .trap_mode()
        .expect("unexpected stvec mode before entering rootserver");
    unsafe { stvec::write(saved_stvec.address(), saved_mode) };
    handle_rootserver_trap(tcb, sstatus)
}

#[inline]
fn handle_rootserver_trap(tcb: &mut Tcb, sstatus: usize) -> ! {
    let cause = scause::read().cause();
    let stval = stval::read();
    let pc = tcb.ctx.pc();

    match cause {
        Trap::Exception(Exception::UserEnvCall) => {
            let syscall_id = tcb.ctx.a(7);
            tcb.ctx.move_next();
            log::info!(
                "[boot] rootserver ecall: pc={:#x}, a7={:#x}, next_pc={:#x}, sstatus={:#x}",
                pc,
                syscall_id,
                tcb.ctx.pc(),
                sstatus,
            );
            shutdown(false)
        }
        Trap::Exception(exception) => {
            log::error!(
                "[boot] rootserver fault: pc={:#x}, stval={:#x}, cause={exception:?}, sstatus={:#x}",
                pc,
                stval,
                sstatus,
            );
            shutdown(true)
        }
        Trap::Interrupt(interrupt) => {
            log::error!(
                "[boot] rootserver interrupt: pc={:#x}, stval={:#x}, cause={interrupt:?}, sstatus={:#x}",
                pc,
                stval,
                sstatus,
            );
            shutdown(true)
        }
    }
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

    for (i, paddr) in (image_start..image_end).step_by(config::PAGE_SIZE).enumerate() {
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
