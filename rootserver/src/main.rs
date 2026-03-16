#![no_std]
#![no_main]

use core::arch::asm;
use sal4_common::BootInfo;

#[unsafe(no_mangle)]
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

    // The first rootserver only proves the user-mode entry/trap path:
    // enter U-mode, execute one ecall, then spin if control returns.
    unsafe {
        asm!("li a7, 0", "ecall");
    }

    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
