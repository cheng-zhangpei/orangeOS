# ch3-3 分时系统与时间片轮转调度

时间片轮转调度的核心机制就在于计时。操作系统的计时功能是依靠硬件提供的时钟中断来实现的。在介绍时钟中断之前，我们先简单介绍一下中断。

在 RISC-V 架构语境下， **中断** (Interrupt) 和我们第二章中介绍的异常（包括程序错误导致或执行 Trap 类指令如用于系统调用的 `ecall` ）一样都是一种 Trap ，但是它们被触发的原因却是不同的。对于某个处理器核而言， 异常与当前 CPU 的指令执行是 **同步** (Synchronous) 的，异常被触发的原因一定能够追溯到某条指令的执行；而中断则 **异步** (Asynchronous) 于当前正在进行的指令，也就是说中断来自于哪个外设以及中断如何触发完全与处理器正在执行的当前指令**无关**

> 发起中断的是一套与处理器执行指令无关的电路（从时钟中断来看就是简单的计数和比较器），这套电路仅通过一根导线接入处理器。当外设想要触发中断的时候则输入一个高电平或正边沿，处理器会在每执行完一条指令之后检查一下这根线，看情况决定是继续执行接下来的指令还是进入中断处理流程。也就是说，大多数情况下，指令执行的相关硬件单元和可能发起中断的电路是完全独立 **并行** (Parallel) 运行的，它们中间只有一根导线相连。这很好理解我们之前408学过了



riscv中断表

| Interrupt | Exception Code | Description                   |
| --------- | -------------- | ----------------------------- |
| 1         | 1              | Supervisor software interrupt |
| 1         | 3              | Machine software interrupt    |
| 1         | 5              | Supervisor timer interrupt    |
| 1         | 7              | Machine timer interrupt       |
| 1         | 9              | Supervisor external interrupt |
| 1         | 11             | Machine external interrupt    |

- **软件中断** (Software Interrupt)：由软件控制发出的中断=> 主动Trap或者是内部中断
- **时钟中断** (Timer Interrupt)：由时钟电路发出的中断
- **外部中断** (External Interrupt)：由外设发出的中断



中断和特权级之间是有相互关系的，在判断中断是否会被屏蔽的时候，有以下规则：

- 如果中断的特权级低于 CPU 当前的特权级，则该中断会被屏蔽，不会被处理；
- 如果中断的特权级高于与 CPU 当前的特权级或相同，则需要通过相应的 CSR 判断该中断是否会被屏蔽。

以内核所在的 S 特权级为例，中断屏蔽相应的 CSR 有 `sstatus` 和 `sie` 。`sstatus` 的 `sie` 为 S 特权级的中断使能，能够同时控制三种中断，如果将其清零则会将它们全部屏蔽。即使 `sstatus.sie` 置 1 ，还要看 `sie` 这个 CSR，它的三个字段 `ssie/stie/seie` 分别控制 S 特权级的软件中断、时钟中断和外部中断的中断使能。比如对于 S 态时钟中断来说，如果 CPU 不高于 S 特权级，需要 `sstatus.sie` 和 `sie.stie` 均为 1 该中断才不会被屏蔽；如果 CPU 当前特权级高于 S 特权级，则该中断一定会被屏蔽。



> 这里我们还需要对第二章介绍的系统调用和异常发生时的硬件机制做一下与中断相关的补充。默认情况下，当中断产生并进入某个特权级之后，在中断处理的过程中同特权级的中断都会被屏蔽。中断产生后，硬件会完成如下事务：
>
> - 当中断发生时，`sstatus.sie` 字段会被保存在 `sstatus.spie` 字段中，同时把 `sstatus.sie` 字段置零，这样软件在进行后续的中断处理过程中，所有 S 特权级的中断都会被屏蔽；
> - 当软件执行中断处理完毕后，会执行 `sret` 指令返回到被中断打断的地方继续执行，硬件会把 `sstatus.sie` 字段恢复为 `sstatus.spie` 字段内的值。
>
> 也就是说，如果不去手动设置 `sstatus` CSR ，在只考虑 S 特权级中断的情况下，是不会出现 **嵌套中断** (Nested Interrupt) 的。嵌套中断是指在处理一个中断的过程中再一次触发了中断。由于默认情况下，在软件开始响应中断前， 硬件会自动禁用所有同特权级中断，自然也就不会再次触发中断导致嵌套中断了。

我们在响应中断的时候嵌套中断在同特权级的中断之间会尽量不打断，这个过程就是中断控制器控制的。**默认不嵌套是为了简化内核编程、减少栈开销、提高系统可预测性。这是硬件 + 操作系统的共同设计决策**



RISC-V 架构要求处理器要有一个内置时钟，其频率一般低于 CPU 主频。此外，还有一个计数器用来统计处理器自上电以来经过了多少个内置时钟的时钟周期。在 RISC-V 64 架构上，该计数器保存在一个 64 位的 CSR `mtime` 中，我们无需担心它的溢出问题，在内核运行全程可以认为它是一直递增的。

> 为啥频率更低其实是因为频率高的话耗电啊，而且有没有必要做那么高的频率？似乎没必要，那么高精度反而会增加能耗

## 抢占式调度

有了时钟中断和计时器，抢占式调度就很容易实现了：

```rust
// os/src/trap/mod.rs

match scause.cause() {
    Trap::Interrupt(Interrupt::SupervisorTimer) => {
        set_next_trigger();
        suspend_current_and_run_next();
    }
}
```

我们只需在 `trap_handler` 函数下新增一个条件分支跳转，当发现触发了一个 S 特权级时钟中断的时候，首先重新设置一个 10ms 的计时器，然后调用上一小节提到的 `suspend_current_and_run_next` 函数暂停当前应用并切换到下一个。

为了避免 S 特权级时钟中断被屏蔽，我们需要在执行第一个应用之前进行一些初始化设置：

```rust
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    trap::init();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}

// os/src/trap/mod.rs

use riscv::register::sie;

pub fn enable_timer_interrupt() {
    unsafe { sie::set_stimer(); }
}
```

- 第 9 行设置了 `sie.stie` 使得 S 特权级时钟中断不会被屏蔽；
- 第 10 行则是设置第一个 10ms 的计时器。

> 这里稍微标注一下，似乎当时学这个位置的很多人都有点疑问，
>
> | **SIE**  | Supervisor Interrupt Enable          | **当前是否开启 S 模式中断**。1 = 开启，0 = 关闭。   |
> | -------- | ------------------------------------ | --------------------------------------------------- |
> | **SPIE** | Supervisor Previous Interrupt Enable | **进入 trap 之前的 SIE 值**，用于 trap 返回时恢复。 |
> | **SPP**  | Supervisor Previous Privilege Mode   | **进入 trap 之前的特权级**：0 = U，1 = S。          |

当 CPU 从 U 或 S 模式进入 S 模式的 trap（比如时钟中断、ecall）：

1. `sstatus.SPIE` = `sstatus.SIE`（保存当前中断使能状态）
2. `sstatus.SIE` = 0（**自动关闭 S 模式中断**，避免嵌套）
3. `sstatus.SPP` = 进入 trap 之前的模式（U 或 S）

这样设计是为了**默认禁止中断嵌套**，简化内核编程。

所以只要是进入S状态的Trap，不管是时钟还是ecall都会禁止嵌套。

| 当前特权级 | 中断是否响应                                                 |
| :--------- | :----------------------------------------------------------- |
| U 模式     | 即使 `SIE=0`，**来自 S 模式的中断仍然会触发 trap**。因为中断的目标特权级是 S，不是 U。 |
| S 模式     | `SIE=0` 时，**S 模式的中断被屏蔽**，不会响应。               |

