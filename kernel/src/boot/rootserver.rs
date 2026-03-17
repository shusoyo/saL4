use crate::{
    cap::threads::{Tcb, ThreadState},
    config,
    syscall::handle_syscall,
};

use riscv::register::{
    scause::{self, Exception, Trap},
    stval,
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

pub fn prepare_rootserver_context(
    tcb: &mut Tcb,
    image: RootserverImage,
    stack_paddr: usize,
    bootinfo_paddr: usize,
    ipc_buffer_paddr: usize,
) {
    let mut ctx = LocalContext::user(image.start);
    *ctx.sp_mut() = stack_paddr + config::ROOTSERVER_STACK_SIZE;
    *ctx.a_mut(0) = bootinfo_paddr;
    *ctx.a_mut(1) = ipc_buffer_paddr;

    tcb.ctx = ctx;
    tcb.state = ThreadState::Running;

    log::info!(
        "[boot] prepared rootserver context: pc={:#x}, sp={:#x}, bootinfo={:#x}, ipc_buffer={:#x}",
        tcb.ctx.pc(),
        tcb.ctx.sp(),
        tcb.ctx.a(0),
        tcb.ctx.a(1),
    );
}

pub fn enter_rootserver(tcb: &mut Tcb) -> ! {
    log::info!("[boot] entering rootserver");

    loop {
        let sstatus = unsafe { tcb.ctx.execute() };
        handle_rootserver_trap(tcb, sstatus)
    }
}

fn handle_rootserver_trap(tcb: &mut Tcb, sstatus: usize) {
    let cause = scause::read().cause();
    let stval = stval::read();
    let pc = tcb.ctx.pc();

    match cause {
        Trap::Exception(Exception::UserEnvCall) => {
            tcb.ctx.move_next();
            handle_syscall(&tcb.ctx);
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
