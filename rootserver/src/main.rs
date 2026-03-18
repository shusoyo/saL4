#![no_std]
#![no_main]

use core::arch::asm;
use sal4_common::{BootInfo, SlotRegion, invocation, sal4_syscall};
use tg_console::log;
use tg_console::println;
use tg_syscall::{self, native::syscall0, native::syscall1, native::syscall5};

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    let bootinfo_ptr: usize;
    let ipc_buffer: usize;

    unsafe {
        asm!(
            "mv {bootinfo}, a0",
            "mv {ipc}, a1",
            bootinfo = out(reg) bootinfo_ptr,
            ipc = out(reg) ipc_buffer,
        );
    }

    let bootinfo = unsafe { &*(bootinfo_ptr as *const BootInfo) };
    let _ = bootinfo.ipc_buffer;
    let _ = ipc_buffer;

    tg_console::init_console(&Console);
    tg_console::set_log_level(option_env!("LOG").or(Some("INFO")));

    log::info!("hello rootserver");
    print_bootinfo_summary(bootinfo, ipc_buffer);
    request_first_frame_retype(bootinfo);

    shutdown();
    unreachable!()
}

fn print_bootinfo_summary(bootinfo: &BootInfo, ipc_buffer: usize) {
    println!("rootserver boot summary");
    println!(
        "  bootinfo       = {:#x}",
        bootinfo as *const BootInfo as usize
    );
    println!("  ipc_buffer(a1) = {:#x}", ipc_buffer);
    println!("  ipc_buffer     = {:#x}", bootinfo.ipc_buffer);
    println!(
        "  cnode bits     = {}",
        bootinfo.init_thread_cnode_size_bits
    );

    print_slot_region("empty", bootinfo.empty);
    print_slot_region("user_image", bootinfo.user_image_frames);
    print_slot_region("untyped", bootinfo.untyped);

    println!("  user_image pages = {}", user_image_frame_count(bootinfo));
    println!("  untyped count    = {}", untyped_count(bootinfo));
}

fn print_slot_region(name: &str, region: SlotRegion) {
    println!(
        "  {name:<12} = [{}, {}), len={}",
        region.start.raw(),
        region.end.raw(),
        slot_region_len(region),
    );
}

fn slot_region_len(region: SlotRegion) -> usize {
    region.end.raw().saturating_sub(region.start.raw())
}

fn user_image_frame_count(bootinfo: &BootInfo) -> usize {
    slot_region_len(bootinfo.user_image_frames)
}

fn untyped_count(bootinfo: &BootInfo) -> usize {
    slot_region_len(bootinfo.untyped)
}

fn request_first_frame_retype(bootinfo: &BootInfo) {
    let service = bootinfo.untyped.start.raw();
    let destination = bootinfo.empty.start.raw();
    let size_bits = 12usize;

    let ret = cap_invoke(
        service,
        invocation::UNTYPED_RETYPE,
        invocation::OBJECT_FRAME,
        destination,
        size_bits,
    );

    if ret == invocation::OK {
        log::info!(
            "first untyped retype ok: service={}, dest={}, object=frame",
            service,
            destination,
        );
    } else {
        log::error!(
            "first untyped retype failed: service={}, dest={}, error={:#x}",
            service,
            destination,
            ret,
        );
    }
}

fn cap_invoke(service: usize, label: usize, arg0: usize, arg1: usize, arg2: usize) -> isize {
    unsafe { syscall5(sal4_syscall::CAP_INVOKE, service, label, arg0, arg1, arg2) }
}

pub struct Console;

impl tg_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe {
            syscall1(sal4_syscall::DEBUG_PUT_CHAR, c as usize);
        }
    }
}

fn shutdown() {
    unsafe {
        syscall0(sal4_syscall::DEBUG_SHUTDOWN);
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    shutdown();

    unreachable!()
}
