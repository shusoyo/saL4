use crate::{cap::threads::Tcb, config};
use sal4_common::sal4_syscall;
use tg_console::log;
use tg_sbi::{console_putchar, shutdown};

pub fn handle_syscall(tcb: &mut Tcb) {
    // a7 寄存器存放 syscall ID
    let syscall_id = tcb.ctx.a(7).into();

    match syscall_id {
        sal4_syscall::DEBUG_SHUTDOWN | sal4_syscall::DEBUG_PUT_CHAR => {
            handle_debug_syscall(tcb, syscall_id)
        }
        sal4_syscall::CAP_INVOKE => crate::cap::invocation::handle_cap_invoke(tcb),

        _ => {
            log::error!(
                "[syscall] unsupported debug syscall id={}, a0={:#x}, pc={:#x}",
                tcb.ctx.a(7),
                tcb.ctx.a(0),
                tcb.ctx.pc(),
            );
            shutdown(true)
        }
    }
}

fn handle_debug_syscall(tcb: &mut Tcb, syscall_id: tg_syscall::SyscallId) {
    match syscall_id {
        sal4_syscall::DEBUG_PUT_CHAR => {
            console_putchar(tcb.ctx.a(0) as u8);
            *tcb.ctx.a_mut(0) = 0;
        }
        sal4_syscall::DEBUG_SHUTDOWN => shutdown(false),
        _ => unreachable!("debug syscall dispatcher received non-debug syscall"),
    }

    log::trace!(
        "[syscall] debug syscall id={} handled (cptr-bits={})",
        syscall_id.0,
        config::ROOT_CNODE_RADIX_BITS,
    );
}
