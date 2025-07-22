



/*
RefCell是一种运行时检查机制，是允许修改不可变变量的，之前的AppManager是内核全局维护的，其中会维护当前内核允许的任务信息
这个信息在内核运行过程中需要其是一个可变变量
RefCell 的特点：
=> 编译时不检查借用规则，运行时检查。
=> 如果违反规则（比如同时借用多个可变引用），会 panic。
=> 更灵活，但也有额外的运行时开销.
*/
use core::cell::{RefCell, RefMut};

pub struct UPSafeCell<T> {
    /// inner data
    inner: RefCell<T>,
}
// 告诉编译器这个结构体可以在线程间安全共享（保证是单核）
unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    // 创建一个 UPSafeCell,每一个这个Cell可以理解为一个不能被多次借用的变量封装体
    pub unsafe fn new(value: T) -> Self {
        Self { inner: RefCell::new(value) }
    }
    // 获取独占访问权
    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}