# Ch3-2 多道程序与协作式调度

我们为啥要写调度？其实核心目的是围绕着硬件资源，尽可能提高资源的利用率。所以本质上就是要在cpu固定资源的情况下去尽可能让他不断的执行任务。

![../_images/multiprogramming.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/multiprogramming.png)

我们这一节其实要做的就是这个协作式的概念了，就是cpu在需要执行中断任务的时候就通过sys_yield让出cpu。内核将很大的权力下放到应用，让所有的应用互相协作来最终达成最大化 CPU 利用率，充分利用计算资源这一终极目标

![../_images/fsm-coop.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/fsm-coop.png)

这些状态的变化本质是由我们TaskManager去控制的，这些状态的转移是我们内核去实际实施，应用去请求来完成的。

> 对于内核第一个app的加载，我们内核在启动的瞬间是内核态，所以我们需要利用一个__restore的sret回到用户态去运行第一个程序，这个时候我们放了一个TrapContext（伪造的，不是真实硬件产生的），这个TrapContext会人为通过sret去到用户态，从而脱离内核态。

```rust
pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_context(
        TrapContext::app_init_context(get_base_i(app_id), USER_STACK[app_id].get_sp()),
    )
}
```

这就是讲一个TrapContext放入栈中，我把当前的context上下文放到栈中，然后跳到用户态，跳到用户态的哪里，我需要在前面给出对应的地址，上面第三行的逻辑很绕，本质上是，我将现在的内核的context放到对应想要跳转的用户的栈中，这样在用户这个app执行完想要ret的时候就可以直接从他的用户栈中pop出对应的上下文。所以这个保存是交错**保存**。



> 我们每次在操作栈的时候内心都要清楚我们现在在操作哪一个栈，比如在内核中，我们往往操作的是内核栈，默认是内核栈，因为你现在在内核的空间，单单sp本身是一个无状态的东西。所以sp往往经常会被os去维护，比如栈帧中sp就是维护对应栈的某一项的位置，现在系统里面有很多个栈，别搞混了