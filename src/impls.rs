/// 控制台实现：通过 SBI 逐字符输出
pub struct Console;

impl tg_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        tg_sbi::console_putchar(c);
    }
}
