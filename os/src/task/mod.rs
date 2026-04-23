pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM], // 这个里面的TCB是不断变化的哦
    current_task: usize,
}

// 本质上就是在启动的时候把所有的app加载为Task，然后设置为Ready放到数组里面去等待调度喽
lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [
            TaskControlBlock {
                task_cx: TaskContext::zero_init(),
                task_status: TaskStatus::UnInit
            };
            MAX_APP_NUM
        ];
        for i in 0..num_app {
            // 将应用程序的上下文装载到TCB中
            tasks[i].task_cx = TaskContext::goto_restore(init_app_cx(i));
            tasks[i].task_status = TaskStatus::Ready;
        }
        TaskManager {
            num_app,
            inner: unsafe { UPSafeCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            })},
        }
    };
}



impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.borrow_mut(); // 哪一个可变借用然后修改内部状态
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.borrow_mut();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // 找到next的task的本质就是我们的调度的过程了，这里实现的算法就是轮转调度算法，找到下一个Ready状态的任务去执行
        // 类似 4 -> 5 -> 0 -> 1 -> 2 -> 3 -> 4 -> ... 这样如果是Ready那么就把这个id返回，这样就可以根据AppManager去进行
        // 取模可以将一个线性的数组打成环形，保证了每一个任务都有机会被调度到去执行
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| {
                inner.tasks[*id].task_status == TaskStatus::Ready
            })
    }
    // 拿到switch的两个参数地址，也就是两个TaskContext的地址，当前正在运行的任务的TaskContext地址和下一个要运行的任务的TaskContext地址
    // 调用unsafe去进行任务切换，切换的过程就是保存当前任务的上下文到当前任务的TaskContext中，
    // 然后从下一个任务的TaskContext中恢复寄存器状态，最后跳转到下一个任务的入口地址去执行
    fn run_next_task(&self) {
        // 找到下一个要运行的任务的id并且且切换到下一个任务去执行
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;

            /*在实际切换之前我们需要手动 drop 掉我们获取到的 TaskManagerInner 的来自 UPSafeCell 的借用标记
            。因为一般情况下它是在函数退出之后才会被自动释放，从而 TASK_MANAGER 的 inner 字段得以回归到未被借用的状态，
            之后可以再借用。如果不手动 drop 的话，编译器会在 __switch 返回时，也就是当前应用被切换回来的时候才 drop，
            这期间我们都不能修改 TaskManagerInner ，甚至不能读（因为之前是可变借用），会导致内核 panic 报错退出。正因如此，
            我们需要在 __switch 前提早手动 drop 掉 inner 。 */

            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(
                    current_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
      fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(
                // 我们显式在启动栈上分配了一个名为 _unused 的任务上下文，
                // 并将它的地址作为第一个参数传给 __switch ，这样保存一些寄存器之后的启动栈栈顶的位置将会保存在此变量中
                &mut _unused as *mut TaskContext,
                next_task_cx_ptr,
            );
        }
        panic!("unreachable in run_first_task!");
    }
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}