use riscv::register::sstatus;
use riscv::register::sstatus::{Sstatus, SPP};

#[repr(C)]
pub struct TrapContext {
    // 这个是32个riscv的通用寄存器
    pub x: [usize; 32],
    // 这两个寄存器在Trap的过程中在返回的时候是会用到的，但是由于可能会有嵌套Trap导致这两个寄存器被覆盖，所以需要保存起来
    pub sstatus: Sstatus, // 这个是触发中断是需要修改的cpu特权级字段
    pub sepc: usize, // 这个字段是CSR寄存器中用来报错中断发送处的地址的
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
        };
        cx.set_sp(sp);
        cx
    }
}