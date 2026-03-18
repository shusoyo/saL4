use tg_syscall::SyscallId as Id;

/// Debug-only syscall: output one byte to the kernel console.
///
/// Calling convention:
/// - `a7 = DEBUG_PUT_CHAR`
/// - `a0 = byte to print`
pub const DEBUG_PUT_CHAR: Id = Id(1);

/// Debug-only syscall: terminate the machine through the kernel.
///
/// Calling convention:
/// - `a7 = DEBUG_SHUTDOWN`
pub const DEBUG_SHUTDOWN: Id = Id(2);

/// Invocation (syscall)
/// Calling convention
/// - `a7 = CAP_INVOKE`
/// - `a0 = service cptr`
/// - `a1 = invocation label`
/// - `a2..a5 = invocation arguments`
pub const CAP_INVOKE: Id = Id(16);
