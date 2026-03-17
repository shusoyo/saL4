#![no_std]
#![no_main]

use core::arch::asm;
use sal4_common::{BootInfo, sal4_syscall};
use tg_console::log;
use tg_console::println;
use tg_syscall::{self, native::syscall0, native::syscall1};

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
    tg_console::set_log_level(option_env!("LOG"));

    log::info!("hello rootserver");
    println!("hello {:?}", bootinfo);

    shutdown();
    unreachable!()
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
