.section .text.entry, "ax", @progbits
    .globl _start
_start:
    # 1. 多核屏蔽逻辑
    # 根据 SBI 标准，进入内核时 a0 = hartid, a1 = dtb_address
    # 如果 a0 != 0，则进入死循环挂起
    li t0, 0
    bne a0, t0, .Lsecondary_loop

    # 2. 设置环境
    # 清除 fp，确保回溯（backtrace）在栈顶终止
    li fp, 0
    
    # 将 hartid 存入 tp 寄存器，方便在 Rust 中通过读取 tp 知道当前核
    mv tp, a0

    # 3. 设置栈指针
    # la 指令加载 boot_stack_top 的地址到 sp
    la sp, boot_stack_top

    # 4. 跳转到 Rust 入口
    # a0, a1 此时依然保持原样，rust_main(hartid: usize, dtb: usize) 可以直接接收
    call rust_main

.Lsecondary_loop:
    wfi
    j .Lsecondary_loop

    # --- 栈空间定义 ---
    .section .boot.stack, "aw", @nobits
    .globl boot_stack
boot_stack:
    # 定义 64KB 的启动栈（4096 * 16）
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top: