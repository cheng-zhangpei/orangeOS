use core::arch::asm;

const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;



/*其实这里系统调用的逻辑非常简单就是将参数给填入到我们qemu模拟出来的寄存器里面就好了 */
fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe { 
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret, // x10寄存器是用来传递系统调用的第一个参数的，同时也是用来接收系统调用返回值的
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret // 这是系统调用返回的内容，也就是说内核处理之后的值我们是会放到这个位置的，但是只有一个64位的寄存器他可以表示的东西很有限啊？？？？
}




/// 功能：将内存中缓冲区中的数据写入文件。
/// 参数：`fd` 表示待写入文件的文件描述符；
///      `buf` 表示内存中缓冲区的起始地址；
///      `len` 表示内存中缓冲区的长度。
/// 返回值：返回成功写入的长度。
/// syscall ID：64
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}
/// 功能：退出应用程序并将返回值告知批处理系统。
/// 参数：`exit_code` 表示应用程序的返回值。
/// 返回值：该系统调用不应该返回。
/// syscall ID：93
pub fn sys_exit(exit_code: i32) -> isize {
    syscall(SYSCALL_EXIT, [exit_code as usize, 0, 0])
}
