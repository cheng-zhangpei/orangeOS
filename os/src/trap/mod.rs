pub(crate) mod context;

use riscv::register::{
    mtvec::TrapMode,
    stvec,
    scause::{
        self,
        Trap,
        Exception,
    },
    stval,
};
use crate::syscall;
use crate::batch::run_next_app;
use core::arch::global_asm;
use crate::syscall::syscall;
use crate::trap::context::TrapContext;

global_asm!(include_str!("trap.S"));

pub fn init() {
    // 声明外部汇编函数__alltraps是我们在汇编中指定的位置，作为所有trap的入口点
    extern "C" { fn __alltraps(); }
    unsafe {
        // 设置中断向量表地址，这里是直接用riscv封装的地址
        // all trap 这个函数其实是保存用户上下文之后转到handler里面去处理具体的trap功能
        // stvec这里其实就是让cpu跳到all_traps这个函数去执行，所有的trap最后都要在这个位置，这个函数的功能就是保存用户上下文之后转到handler里面去处理具体的trap功能
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
// __alltraps的汇编执行完之后就到了这个函数了，这个函数的功能就是根据不同的trap类型来进行不同的处理
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    // 也就是cause之类的的区分异常的办法是由寄存器里面直接读出来？ 如果这样用户态应该也需要知道这些异常才对
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        // 1、主动陷入的异常 
        Trap::Exception(Exception::UserEnvCall) => {
            /*一个由 ecall 指令触发的系统调用，在进入 Trap 的时候，硬件会将 sepc 设置为这条 ecall 指令
            所在的地址（因为它是进入 Trap 之前最后一条执行的指令）。 而在 Trap 返回之后，我们希望应用程序控制流从 ecall 的下一条指令开始执行 */
            cx.sepc += 4;
            // 后面就是我们自己设置的系统调用的规范了
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        // 2、存储错误或者是页错误
        Trap::Exception(Exception::StoreFault) |
        Trap::Exception(Exception::StorePageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            // 出现错误就跳转到下一个的application
            run_next_app();
        }
        // 3、非法指令异常
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            // 出现错误就跳转到下一个的application
            run_next_app();
        }
        _ => {
            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval);
        }
    }
    // 丢回去一个可变的借用
    cx
}



