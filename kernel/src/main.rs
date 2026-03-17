//!  Sal4 os

#![no_std]
#![no_main]
#![deny(warnings, missing_docs)]

// 引入控制台输出宏（print! / println!），由 tg_console 库提供

mod boot;
mod cap;
mod config;
mod impls;
mod syscall;

use impls::Console;

core::arch::global_asm!(include_str!("entry.asm"));
const _: &str = env!("ROOTSERVER_IMAGE_STAMP");
core::arch::global_asm!(include_str!(env!("APP_ASM")));

/// S 态主函数：完成最早期初始化后进入内核 boot 流程。
///
/// 该入口只负责清 `.bss`、初始化控制台与日志，
/// 然后将控制权交给 `boot::bootstrap()`。
#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    // locate 中 __end 符号不一定是 4kB 对齐的
    let locate = tg_linker::KernelLayout::locate();
    // clear bss
    unsafe { locate.zero_bss() };

    // 第二步：初始化控制台输出（使 print!/println! 可用）
    tg_console::init_console(&Console);
    tg_console::set_log_level(option_env!("LOG"));
    tg_console::test_log();

    // Enter the kernel boot flow. From this point on the kernel should not
    // return to the early Rust entry except through temporary debug exits.
    boot::bootstrap()
}

/// language item
/// panic 处理函数。
///
/// `#![no_std]` 环境下必须自行实现。发生 panic 时以异常状态关机。
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    tg_sbi::shutdown(true) // true 表示异常关机
}
