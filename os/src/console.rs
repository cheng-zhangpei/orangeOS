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

// $fmt: literal：匹配一个字面量字符串，比如 "hello"、"value = {}"。
// $(, $($arg: tt)+)?：这是一个可选的模式（? 表示 0 次或 1 次）。
// 开头的 ,：表示如果后面有参数，需要先写一个逗号。
// $($arg: tt)+：匹配一个或多个 token tree（tt 可以匹配任何表达式、类型、标识符等），每个 arg 捕获一个参数。
// + 表示至少一个，? 把整个 , 参数列表 变成可选的。
// 也就是说，print! 可以写成两种形式：
// print!("hello") —— 没有额外参数，只匹配 $fmt
// print!("value = {}", x) —— 匹配 $fmt 后，再匹配 , 和参数列表 x

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        // 其实宏内部还是调用了之前使用sbi写的print函数
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}