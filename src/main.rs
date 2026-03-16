//!  Sal4 os

#![no_std]
#![no_main]
#![cfg_attr(target_arch = "riscv64", deny(warnings, missing_docs))]
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]

// 引入控制台输出宏（print! / println!），由 tg_console 库提供

mod boot;
mod cap;
mod config;
mod impls;
mod utils;

use impls::Console;

// riscv 库：访问 RISC-V 控制状态寄存器（CSR），如 scause
// use riscv::register::*;

use tg_console::log;
use tg_sbi::shutdown;

#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("entry.asm"));

/// S 态主函数：打印 "Hello, world!" 并关机。
///
/// 通过 SBI 的 `console_putchar` 逐字节输出字符串，
/// 然后调用 `shutdown` 正常关机退出 QEMU。
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

    // boot
    boot::bootstrap();

    log::info!("hello, world");

    shutdown(false) // false 表示正常关机
}

/// language item
/// panic 处理函数。
///
/// `#![no_std]` 环境下必须自行实现。发生 panic 时以异常状态关机。
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    shutdown(true) // true 表示异常关机
}

/// 非 RISC-V64 架构的占位模块。
///
/// 提供 `main` 等符号，使得在主机平台（如 x86_64）上也能通过编译，
/// 满足 `cargo publish --dry-run` 和 `cargo test` 的需求。
#[cfg(not(target_arch = "riscv64"))]
mod stub {
    /// 主机平台占位入口
    #[unsafe(no_mangle)]
    pub extern "C" fn main() -> i32 {
        0
    }

    /// C 运行时占位
    #[unsafe(no_mangle)]
    pub extern "C" fn __libc_start_main() -> i32 {
        0
    }

    /// Rust 异常处理人格占位
    #[unsafe(no_mangle)]
    pub extern "C" fn rust_eh_personality() {}
}
