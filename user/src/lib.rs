#![no_std]
#![feature(linkage)] // 这个玩意儿是用来声明link的方式[linkage = "weak"]的意思是：如果链接时没有其他定义，则使用这个定义；否则使用强定义

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;


/*这里补充一下，_start 函数定义的是进程的初始化装载到内存中需要进行的初始化工作
ok这里就是用户库的入口，用户库在我们的定义中是与os进程相隔离的进程
*/
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    clear_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

//[linkage = "weak"]的意思是：如果链接时没有其他定义，则使用这个定义；否则使用强定义也就是其他的main
// 这样设计也是为了不会发生main的链接冲突
#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> i32 {
    panic!("Cannot find main!");
}

// clear当前进程的bss段
fn clear_bss() {
    unsafe extern "C" {
        fn start_bss();
        fn end_bss();
    }
    (start_bss as usize..end_bss as usize).for_each(|addr| unsafe {
        (addr as *mut u8).write_volatile(0);
    });
}

use syscall::*;

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}