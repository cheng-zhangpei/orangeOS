// rustup target add thumbv7em-none-eabihf =>“我要为 ARM Cortex-M4/M7 架构的目标平台编译代码，所以请给我安装适用于这个平台的标准库。” 
// 这里詪有意思哦，指定架构平台？这样看来rust的架构兼容性真的做得非常好
// 不适用c标准库 以及 c runtime
#![no_std]
// 这个语言项是用于标记，main函数得运行是需要底层得runtime作为支撑的，也就是有点类似于程序装载器一样，需要指定程序的入口
#![no_main]

#[unsafe(no_mangle)]
// 我们重写的操作系统程序入口
// 这个函数是直接由os装载器直接调用的，我估计这个装载器后面还需要自己写
pub extern "C" fn _start() -> ! {
    loop {}
}

/// This function is called on panic.
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}