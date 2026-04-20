# char1-4：关于内核的一些知识

## 栈帧结构

![../_images/StackFrame.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/StackFrame.png)

这是栈帧的具体结构，我们寄存器fp、sp会维护这个栈的结构，栈帧中ra是这个函数调用完后返回的地址，prev fp其实就是上一个栈帧的起点，下面就是一些保存的上下文信息之类的了。这个是riscv中的C语言的函数调用规约。

需要注意的点是sp是低地址，所以我们在操作栈的时候往往是sp + imm这样的形式来做的。这个栈地址的增长方向是由riscv架构决定的，因为这涉及到ISA的详细电路设计

栈上多个 `fp` 信息实际上保存了一条完整的函数调用链，通过适当的方式我们可以实现对函数调用关系的跟踪。

```asm
# os/src/entry.asm
    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top
    call rust_main

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound: # boot_stack_lower_bound就是栈的下限
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top: 
```

在十一行，我们开了一段的空间给栈用，所以对于这个进程来说栈空间是固定的，这就是为啥会有栈溢出这种说法了，boot_stack_lower_bound与boot_stack_top之间就是栈空间了。

在栈设置好了之后就可以回到rust的main函数中写了，后面的内容就可以由rust接管了

> **ABI** 是 **Application Binary Interface**（应用程序二进制接口）的缩写。它定义了二进制层面如何调用函数、传递参数、处理返回值、布局数据结构、进行系统调用等。简单说，**ABI 是编译后的代码之间的约定**，让不同编译器（甚至不同语言）生成的代码能互相协作。

> 在riscv的视角，os位于supervisor特权级，SBI位于machine特权级。rust社区其实已经在cargo里面封装了将rust与我们的rustSBI链接在一起的代码，在编译的之后给编译器指定sbi的代码位置就好了，链接器会自动将其与我们的内核链接到一起

