# Char1：内核的第一条代码的实践
## 第一条内核指令
```asm
 # os/src/entry.asm
     .section .text.entry # 也就是这个汇编中的内容全部放到.text.entry段中
     .globl _start # 将这个_start 符号放到全局段中
 _start:
 # 符号因此符号 _start 的地址即为第 5 行的指令所在的地址
     li x1, 100 # li 是 Load Immediate 的缩写，也即将一个立即数加载到某个寄存器
```
一般情况下，所有的代码都被放到一个名为 .text 的代码段中，这里我们将其命名为 .text.entry 从而区别于其他 .text 的目的在于我们想要确保该段被放置在相比任何其他代码段更低的地址上。这样，作为内核的入口点，这段指令才能被最先执行。
我们在 main.rs 中嵌入这段汇编代码，这样 Rust 编译器才能够注意到它，不然编译器会认为它是一个与项目无关的文件：
```c
// main.rs
global_asm!(include_str!("entry.asm"));
// link.ld
// 指定目标架构为 RISC-V
OUTPUT_ARCH(riscv)

// 程序入口点符号：_start
ENTRY(_start)

// 定义符号 BASE_ADDRESS = 0x80200000（通常 RISC-V 内核起始地址）
BASE_ADDRESS = 0x80200000;

// 开始段布局
SECTIONS
{
    // 当前位置计数器设置为 BASE_ADDRESS
    . = BASE_ADDRESS;
    // . 表示当前地址，也就是链接器会从它指向的位置开始往下放置从输入的目标文件中收集来的段
    skernel = .;

    // 代码段起始符号
    stext = .;
    // 代码段 -> 这个里面才是具体的链接起来的代码段
    .text : { // 冒号前面是要链接生成段的名字
        // 首先放入 .text.entry 节（通常是入口汇编代码）
        *(.text.entry)
        // 然后放入所有 .text 和 .text.* 节
        *(.text .text.*)
    }

    // 4K 对齐
    . = ALIGN(4K);
    // 代码段结束符号
    etext = .;
    // 只读数据段起始
    srodata = .;
    .rodata : {
        // 所有只读数据节
        *(.rodata .rodata.*)
        // 以及小只读数据（RISC-V 小数据段）
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    // 数据段起始
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    // BSS 段（未初始化数据）
    .bss : {
        // 首先放入栈节（通常是内核栈）
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    // 内核结束符号
    ekernel = .;

    // 丢弃的节（不需要链接进最终镜像）
    /DISCARD/ : {
        *(.eh_frame)
    }
}
```
上面是内核代码的运行脚本，这是内存布局的链接脚本用于指导链接器工作的。
> 动态链接和静态链接：静态链接比较简单就是全量加载入内存，动态链接

上面得到的内核可执行文件完全符合我们对于内存布局的要求，但是我们不能将其直接提交给 Qemu ，因为它除了实际会被用到的代码和数据段之外还有一些多余的元数据，这些元数据无法被 Qemu 在加载文件时利用，且会使代码和数据段被加载到错误的位置。如下图所示：
![](image-3.png)
所以其实并不难主要是我们如果直接编译为ETL文件，Rust会默认加上元数据在头尾，所以我们需要去掉这个头尾，让他直接加载我们的section。我们可以在命令行里面加参数去掉这个元数据
