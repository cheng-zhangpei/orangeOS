use crate::sbi::console_putchar;
use core::fmt::{self, Write};

struct Stdout;
// 这里为Stdout实现core中的这个Write的trait
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as usize);
        }
        // 返回成功
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    // unwrap() 是只要出错就panic
    // write_fmt是write_str的封装，这里write_fmt是有一步格式上的解包
    // fmt::Arguments是一个编译器提供的宏，用于输出格式化对象
    Stdout.write_fmt(args).unwrap();
}
// $fmt: literal：匹配一个字符串字面量
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}