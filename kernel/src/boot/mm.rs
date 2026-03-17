use crate::{cap::Slot, config};

use core::{
    mem::{align_of, size_of},
    ptr::NonNull,
};

use super::RootserverImage;

pub struct BootArena {
    current: usize,
    end: usize,
}

impl BootArena {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            current: Self::align_up(start, config::PAGE_SIZE),
            end,
        }
    }

    pub fn align_up(x: usize, align: usize) -> usize {
        assert!(align.is_power_of_two(), "align must be 2^n");
        x.checked_add(align - 1).expect("overflow") & !(align - 1)
    }

    pub fn alloc_typed<T>(&mut self) -> NonNull<T> {
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

    pub fn alloc_slots(&mut self, count: usize) -> NonNull<Slot> {
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

    pub fn alloc_page_paddr(&mut self) -> usize {
        self.current = Self::align_up(self.current, config::PAGE_SIZE);
        let paddr = self.current;
        self.current += config::PAGE_SIZE;
        assert!(self.current <= self.end, "Out of memory during bootstrap");

        unsafe {
            core::ptr::write_bytes(paddr as *mut u8, 0, config::PAGE_SIZE);
        }

        paddr
    }

    pub fn cursor(&self) -> usize {
        self.current
    }
}

pub struct UntypedStream {
    pub current_paddr: usize,
    pub end_paddr: usize,
}

impl Iterator for UntypedStream {
    type Item = (usize, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let remain = self.end_paddr.saturating_sub(self.current_paddr);
        if remain < config::PAGE_SIZE {
            return None;
        }

        // Keep untyped in 2^n sizes (seL4-like model), but use clear arithmetic checks.
        let s_paddr_bits = self.current_paddr.trailing_zeros();
        let remain_bits = remain.ilog2();
        let size_bits = s_paddr_bits.min(remain_bits);

        let paddr = self.current_paddr;
        self.current_paddr += 1usize << size_bits;
        Some((paddr, size_bits as u8))
    }
}

pub fn probe_free_memory(rootserver: RootserverImage) -> (usize, usize) {
    // The rootserver image is already copied to its fixed run address by
    // AppMeta::iter(), so boot-time free memory starts after whichever is later:
    // the kernel image end or the rootserver image end.
    let locate = tg_linker::KernelLayout::locate();
    let kernel_end = locate.end();
    let free_memory_start = BootArena::align_up(kernel_end.max(rootserver.end), config::PAGE_SIZE);
    let free_memory_end = kernel_end + config::EARLY_BOOT_MEMORY_SIZE;

    assert!(
        free_memory_start <= free_memory_end,
        "rootserver image at [{:#x}, {:#x}) exceeds the early-boot memory window [{:#x}, {:#x})",
        rootserver.start,
        rootserver.end,
        kernel_end,
        free_memory_end,
    );

    (free_memory_start, free_memory_end)
}
