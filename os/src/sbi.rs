
pub fn console_putchar(c : usize) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}
/*
failure:表示是否发生错误而强制关机
system_reset：会让CPU陷入机器特权级，并执行关机指令

=>  只需将要调用功能的拓展 ID 和功能 ID 分别放在 a7 和 a6 寄存器中，并按照 RISC-V 调用规范将参数放置在其他寄存器中，
    随后执行 ecall 指令即可。这会将控制权转交给 RustSBI 并由 RustSBI 来处理请求，处理完成后会将控制权交还给内核。
    返回值会被保存在 a0 和 a1 寄存器中
 */
pub fn shutdown(failure : bool) -> !{
    // 将这些模块中的这些内容拿出来
    use sbi_rt::{system_reset, NoReason, Shutdown, SystemFailure};
    if !failure {
        system_reset(Shutdown, NoReason);
    } else {
        system_reset(Shutdown, SystemFailure);
    }
    // 告诉编译器这条指令是不能执行的
    unreachable!()
}


const SBI_SET_TIMER: usize = 0;

pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, timer, 0, 0);
}

// os/src/timer.rs

use crate::config::CLOCK_FREQ;
const TICKS_PER_SEC: usize = 100;
// 每次出发时钟中断的间隔
pub fn set_next_trigger() {
    // CLOCK_FREQ / TICKS_PER_SEC的意思其实就是10ms内会有多少次脉冲
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}