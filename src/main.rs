#![no_std]
// 这个语言项是用于标记，main函数得运行是需要底层得runtime作为支撑的，也就是有点类似于程序装载器一样，需要指定程序的入口
#![no_main]
use core::arch::global_asm;

mod lang_item;
mod sbi;
mod console;

// 将内联汇编嵌入代码
global_asm!(include_str!("entry.asm"));
#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    println!("Hello, world!");
    // 从这里开始就可以出动的触发panic了
    panic!("Shutdown machine!");
}


fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}
