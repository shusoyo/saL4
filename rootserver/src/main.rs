#![no_std]
#![no_main]

use core::arch::asm;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
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
