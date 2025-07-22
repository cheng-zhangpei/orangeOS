use lazy_static::lazy_static;
use lazy_static::*;
use crate::trap::context::TrapContext;
use core::arch::asm;
use crate::sync::UPSafeCell;
// 用户栈大小
const USER_STACK_SIZE: usize = 4096 * 2;
// 内核栈大小
const KERNEL_STACK_SIZE: usize = 4096 * 2;

// 最大运行提交应用数量
const MAX_APP_NUM: usize = 16;
// 应用链接地址
const APP_BASE_ADDRESS: usize = 0x80400000;
// 应用大小限制
const APP_SIZE_LIMIT: usize = 0x20000;


/*
应用加载器
*/
struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}
// lazy_static! 是一个 宏（macro） ，允许在 运行时初始化一个 静态变量（static） 。
// 所以这个变量是运行时才会被加载出的，初试状态下并不会初始化这个变量
impl AppManager {
    pub fn print_app_info(&self) {
        // 打印应用程序 i 的起始地址和结束地址
        // self.app_start[i] 是第 i 个程序的起始地址
        // self.app_start[i + 1] 是下一个程序的起始地址，也就是当前程序的结束地址
        println!("[kernel] num_app = {}", self.num_app);
        for i in 0..self.num_app {
            println!("[kernel] app_{} [{:#x}, {:#x})", i, self.app_start[i], self.app_start[i + 1]);
        }
    }
    // 外界主动调用这个函数来上传某一个应用
    unsafe fn load_app(&self, app_id: usize) {
        // 检查 app_id 是否合法（不能超过应用程序总数）
        if app_id >= self.num_app {
            panic!("All applications completed!");
        }
        println!("[kernel] Loading app_{}", app_id);
        // clear icache
        //  使用内联汇编执行 fence.i 指令，清除指令缓存（I-Cache），这里的指令缓存指的是上一个应用的指令缓存
        //  在 RISC-V 架构中，当内存内容被修改后，CPU 可能还在使用旧的缓存指令，需要刷新。在riscv架构下是指令缓存和数据缓存分离的
        asm!("fence.i");
        // 将应用程序的内存区域清零
        // APP_BASE_ADDRESS 是应用程序的加载地址
        // APP_SIZE_LIMIT 是应用程序的最大大小
        // from_raw_parts_mut 创建一个可变的原始内存切片
        // fill(0) 将这段内存全部填充为 0
        core::slice::from_raw_parts_mut(
            APP_BASE_ADDRESS as *mut u8,
            APP_SIZE_LIMIT
        ).fill(0);
        // 把这个应用程序的源码从内存切片里面取出来
        let app_src = core::slice::from_raw_parts(
            self.app_start[app_id] as *const u8,
            self.app_start[app_id + 1] - self.app_start[app_id]
        );
        // 准备将程序复制到目标地址（APP_BASE_ADDRESS）
        // 大小和源程序相同
        let app_dst = core::slice::from_raw_parts_mut(
            APP_BASE_ADDRESS as *mut u8,
            app_src.len()
        );
        app_dst.copy_from_slice(app_src);
    }

    pub fn get_current_app(&self) -> usize { self.current_app }
    // 让内核转移到下一个应用程序
    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }
}

lazy_static! {
    // 这里就是创建一个不能被多次借用的AppManager变量
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe { UPSafeCell::new({
        // _num_app是在连接器里设计的应用初始地址,定义在link_app.S的开头
        extern "C" { fn _num_app(); }
        let num_app_ptr = _num_app as usize as *const usize;
        // read_volatile：从num_app_ptr这个指针里面把值读出来
        let num_app = num_app_ptr.read_volatile();
        // 创建一个数组，用来保存每个应用程序的起始地址。
        let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
        // 从 num_app 后面的地址开始，读取 num_app + 1 个 usize，构成一个切片。
        let app_start_raw: &[usize] =  core::slice::from_raw_parts(
            num_app_ptr.add(1), num_app + 1
        );
        // 把这个切片的内容复制到 app_start 数组中。
        app_start[..=num_app].copy_from_slice(app_start_raw);
        AppManager {
            num_app,
            current_app: 0,
            app_start,
        }
    })};
}
// 确保这个结构体的起始地址是 4096 的整数倍
#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}
// 将两个栈初始化出来
static USER_STACK: UserStack = UserStack { data: [0; USER_STACK_SIZE] };
static KERNEL_STACK:KernelStack = KernelStack{data:[0;KERNEL_STACK_SIZE]};


impl KernelStack {
    fn get_sp(&self) -> usize {
        // 其实就是将起始指针往后移动栈的size
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    pub fn push_context(&self, cx :TrapContext) -> &'static mut TrapContext {
        let cx_ptr = (self.get_sp() - size_of::<TrapContext>()) as *mut TrapContext;
        unsafe{*cx_ptr = cx;}
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();
    unsafe {
        app_manager.load_app(current_app);
    }
    app_manager.move_to_next_app();
    drop(app_manager);
    // before this we have to drop local variables related to resources manually
    // and release the resources
    extern "C" { fn __restore(cx_addr: usize); }
    unsafe {
        __restore(KERNEL_STACK.push_context(
            TrapContext::app_init_context(APP_BASE_ADDRESS, USER_STACK.get_sp())
        ) as *const _ as usize);
    }
    panic!("Unreachable in batch::run_current_app!");
}