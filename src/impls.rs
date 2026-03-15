/// 控制台实现：通过 SBI 逐字符输出
pub struct Console;

impl tg_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        tg_sbi::console_putchar(c);
    }
}

/// 系统调用上下文实现：处理 IO 与 Process 相关系统调用。
pub struct SyscallContext;

impl tg_syscall::IO for SyscallContext {
    fn write(&self, _caller: tg_syscall::Caller, fd: usize, buf: usize, count: usize) -> isize {
        match fd {
            tg_syscall::STDOUT | tg_syscall::STDDEBUG => {
                print!("{}", unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        buf as *const u8,
                        count,
                    ))
                });
                count as isize
            }
            _ => {
                tg_console::log::error!("unsupported fd: {}", fd);
                -1
            }
        }
    }
}

impl tg_syscall::Process for SyscallContext {
    #[inline]
    fn exit(&self, _caller: tg_syscall::Caller, _status: usize) -> isize {
        0
    }
}
