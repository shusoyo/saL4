//!  Sal4 os

#![no_std]
#![no_main]
#![cfg_attr(target_arch = "riscv64", deny(warnings, missing_docs))]
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]

// 引入控制台输出宏（print! / println!），由 tg_console 库提供
#[macro_use]
extern crate tg_console;

mod boot;
mod cap;
mod config;
mod impls;
mod syscall_gate;

use impls::{Console, SyscallContext};

use cap::threads::Tcb;
use syscall_gate::TrapAction;

use tg_console::log;
use tg_sbi::shutdown;

#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("entry.asm"));
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!(env!("APP_ASM")));

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

    tg_syscall::init_io(&SyscallContext);
    tg_syscall::init_process(&SyscallContext);

    run_apps();

    log::info!("hello world");

    shutdown(false) // false 表示正常关机
}

#[cfg(target_arch = "riscv64")]
fn run_apps() {
    for (i, app) in tg_linker::AppMeta::locate().iter().enumerate() {
        let entry = app.as_ptr() as usize;
        log::info!("load app{} at {:#x}", i, entry);

        let mut user_stack = [0usize; 512];
        let sp = user_stack.as_mut_ptr() as usize + core::mem::size_of_val(&user_stack);

        let mut tcb = Tcb::new(i, entry, sp, 0x100 + i);

        while tcb.alive {
            unsafe { tcb.ctx.execute() };
            match syscall_gate::handle_trap(&mut tcb) {
                TrapAction::Continue => continue,
                TrapAction::Exit | TrapAction::Killed => break,
            }
        }

        let _ = core::hint::black_box(&user_stack);
        println!();
    }
}

#[cfg(not(target_arch = "riscv64"))]
fn run_apps() {}

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
