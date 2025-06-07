#![no_std]
// 这个语言项是用于标记，main函数得运行是需要底层得runtime作为支撑的，也就是有点类似于程序装载器一样，需要指定程序的入口
#![no_main]

#[unsafe(no_mangle)]
// 我们重写的操作系统程序入口
// 这个函数是直接由os装载器直接调用的，我估计这个装载器后面还需要自己写，这个函数会被bootLoader直接调用
static HELLO: &[u8] = b"Hello World!";
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8; // x86 架构下 VGA 文本模式显存的起始地址。
    //as *mut u8 是把这个整数地址转换为一个指向 u8 类型的可变指针

    for (i, &byte) in HELLO.iter().enumerate() {
        //因为我们在操作raw指针（*mut u8），这在 Rust 中属于“不安全代码块”，必须用 unsafe 包裹。
        // 这里面只能做五件事情，其实就是相当于告诉编译器这里的野指针是有意义的，不会导致 data corruption
        unsafe {
            // 这里就是往指针所在位置赋值，先复制值，再设置颜色
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }
    loop {}
}

/// This function is called on panic.
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}