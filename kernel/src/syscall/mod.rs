use sal4_common::sal4_syscall;
use tg_kernel_context::LocalContext;
use tg_sbi::{console_putchar, shutdown};

pub fn handle_syscall(ctx: &LocalContext) {
    // a7 寄存器存放 syscall ID
    let syscall_id = ctx.a(7).into();

    match syscall_id {
        sal4_syscall::DEBUG_PUT_CHAR => {
            console_putchar(ctx.a(0) as u8);
        }
        sal4_syscall::DEBUG_SHUTDOWN => shutdown(false),
        _ => {
            unimplemented!()
        }
    }
}
