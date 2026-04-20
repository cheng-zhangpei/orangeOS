# Char1：从逻辑到系统调用


![alt text](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/lib-os-detail.png)
  这个是os运行的结构，QEMU在第一章会将我们的用户APP与OS的镜像全量的加载并且运行。整体的架构分为了用户态和内核态，用户态的应用通过系统调用陷入内核执行操作。对于os内核，他会先初始化栈空间和bss段清零等操作，然后跳转到用户态的APP执行。
  应用打印字符串等操作都是通过系统调用去实现的，不需要直接与硬件交互。
图中的S-Mode和M-Mode是RISC-V 处理器架构中的两种特权级别。S-Mode 指的是 Supervisor 模式，是操作系统使用的特权级别，可执行特权指令等。M-Mode是 Machine模式，其特权级别比S-Mode还高，可以访问RISC-V处理器中的所有系统资源。
> All problems in computer science can be solved by another level of indirection。

> 库操作系统（Library OS，LibOS）LibOS 以函数库的形式存在，为应用程序提供操作系统的基本功能。它最早来源于 MIT PDOS研究小组在1996年左右的Exokernel（外核）操作系统结构研究。Frans Kaashoek 教授的博士生Dawson Engler 提出了一种与以往操作系统架构大相径庭的 Exokernel（外核）架构设计 1 ，即把传统的单体内核分为两部分，一部分以库操作系统的形式（即 LibOS）与应用程序紧耦合以实现传统的操作系统抽象，并进行面向应用的裁剪与优化；另外一部分（即外核）仅专注在最基本的安全复用物理硬件的机制上，来给 LibOS 提供基本的硬件访问服务。这样的设计思路可以针对应用程序的特征定制 LibOS ，达到高性能的目标。这种操作系统架构的设计思路比较超前，对原型系统的测试显示了很好的性能提升。但最终没有被工业界采用，其中一个重要的原因是针对特定应用定制一个LibOS的工作量大，难以重复使用。人力成本因素导致了它不太被工业界认可。
> 所以人力成本其实也是我们在进行系统开发的时候一个很重要的因素。如何考虑系统的兼容性

## 1 Bare-Mental 
### 1.1 移除

#### 1.1.1 移除STD标准库与panic_handler
对于移除标准库其实很容易只要在main.rs中加入#![no_std]告诉编译器不要链接标准库就好了。
在标准库 std 中提供了关于 panic! 宏的具体实现，其大致功能是打印出错位置和原因并杀死当前应用。但本章要实现的操作系统不能使用还需依赖操作系统的标准库std，而更底层的核心库 core 中只有一个 panic! 宏的空壳，并没有提供 panic! 宏的精简实现。因此我们需要自己先实现一个简陋的 panic 处理函数
>#[panic_handler] 是一种编译指导属性，用于标记核心库core中的 panic! 宏要对接的函数（该函数实现对致命错误的具体处理）。本质就是将宏和我们的函数拼接在一起咯？

#### 1.1.2 main函数的移除
我们实际上使用的main函数是core中实现的，本质的功能其实是需要对应用程序进行一些初始化操作然后跳到这个程序的入口。我们需要在lang_item里面实现一个语义项和之前的panic一样，写一个start语义项。我们在 main.rs 的开头加入设置 #![no_main] 告诉编译器我们没有一般意义上的 main 函数，并将原来的 main 函数删除。在失去了 main 函数的情况下，编译器也就不需要完成所谓的初始化工作了。

我们删除了很多的功能，也就是原来core中的一些固有的功能 。现在我们要以我们的方式去重塑这些功能
> 在rust编程中，我们不管是lib还是bin（一定会生成一个main.rs），都需要利用cargo去进行创建，否则不会被编译器识别到。
> lib模块的根是mod.rs，bin模块的根是main.rs

​	
### QEMU
####  QEMU基础
QEMU本质是一个解释器 + 二进制翻译器 + 硬件模型库。本质上是动态二进制翻译，以代码块为单位将虚拟机或者是我的RISCV内核指令翻译为现在宿主机的指令。
QEMU 内部有一个设备库，每个设备都是一个 C 结构体 + 回调函数：
```c
struct VirtIOBlock {
    uint8_t status;
    void (*write)(uint32_t addr, uint8_t data);
    uint8_t (*read)(uint32_t addr);
    // QEMU 主循环会调用这些函数
};
```
当你的内核向某个内存地址写数据时：
- 普通地址：直接读写 QEMU 内部的内存数组
- MMIO 地址：触发设备回调函数（比如向虚拟硬盘写数据）
往下是一个事件循环：
```c
while (!poweroff) {
    // 1. 执行 CPU 指令（可能触发设备访问）
    translate_and_execute_one_insn();
    // 2. 处理定时器中断
    if (timer_expired()) {
        raise_irq(TIMER_INTERRUPT);
    }
    // 3. 处理网络包（如果模拟了网卡）
    if (tap_fd_has_data()) {
        virtio_net_handle_rx();
    }
    // 4. 刷新显示（如果模拟了显卡）
    if (vga_updated()) {
        update_window();
    }
}

```
也就是对于操作系统的模拟本质是一个事件循环，所有的设备都需要共享一个主循环，让后将我们的从MMIO中拿到的内容转给对应的设备。
外部事件如何到达QEMU：
- 定时器：Linux的timerfd_create()，内核定期唤醒QEMU

- 网络：Linux的TUN/TAP设备，网卡驱动把包写入这个fd

- 键盘：VNC/SPICE/SDL，通过socket或X11事件

设备模拟本质：每个设备是一个状态机 + fd回调函数。QEMU主循环是事件驱动的，不是轮询。
#### 内核镜像的装载
- 加载
1、分配一个 uint8_t ram[128MB] 数组
2、读取 your_kernel.bin
3、解析 ELF 格式，找到代码段位置（通常是 0x80200000）
4、把代码段复制到 ram[0x80200000]
- 初始化cpu
```c
cpu.pc = 0x80200000;  // 程序计数器指向入口
cpu.regs[0] = 0;      // x0 永远是 0
cpu.mode = SUPERVISOR; // 设置为 S 模式
```
- 执行第一条指令
```c
while (1) {
    // 从 ram[cpu.pc] 读 4 字节
    uint32_t insn = fetch_insn(cpu.pc);  
    cpu.pc += 4;
    execute_insn(insn);  // 根据 opcode 执行不同 C 代码
}
```
execute_insn本质上就是一个switch函数，根据操作码执行不同的指令，所以本质上就是翻译指令将指令翻译成软件模拟的指令。
```c
void execute_insn(uint32_t insn) {
    switch (insn >> 25) {
        case 0b0000011: // LOAD 指令
            handle_load(insn);
            break;
        case 0b0010011: // 算术运算
            handle_alu(insn);
            break;
        case 0b1101111: // JAL 跳转
            handle_jump(insn);
            break;
        // ... 几十个 case
    }
}

```

- 执行特权指令
当你的内核执行 ecall（系统调用）时：
```c
case 0b1110011: // SYSTEM 指令
    if ((insn >> 20) & 1 == 0) { // ecall
        handle_ecall();
    }
```
handle_ecall 会根据当前模式跳转到 OpenSBI 的代码（QEMU 的 -bios 参数指定的固件）。

#### 为啥内核不需要修改就能在真实硬件跑
因为 QEMU 的 virt 机器模型是真实硬件的规范实现：
- 内存映射一致（CLINT、PLIC 等地址固定）
- 中断控制器行为相同
- SBI 调用接口一致
- 真实 SiFive U740 芯片的 RISC-V 核，执行你的 ecall 时，也会陷入 M 模式，然后跳转到 OpenSBI。
- 性能关键：TCG（Tiny Code Generator）
解释执行很慢（每条指令都要 switch-case），QEMU 会用 TCG 提速(这个就是动态二进制翻译啊)：
```text
RISC-V 指令序列
    ↓ [翻译块，约 32 条指令]
x86 机器码（缓存在内存）
    ↓
直接执行 x86 代码（没有解释开销）
这就是"二进制翻译"，让 QEMU 能达到真机 20-50% 的速度。
```
#### 设备操作捕获
内存映射I/O（MMIO）+ 页表陷阱。
关键点：你的镜像不需要定制，QEMU通过内存访问监控自动捕获。

```c
// QEMU初始化时注册MMIO区域
memory_region_init_io(&pic_io, NULL, &pic_ops, pic, "pic", 0x1000);
memory_region_add_subregion(address_space, 0x08000000, &pic_io);
// 注册的操作表
static const MemoryRegionOps pic_ops = {
    .read = pic_read,   // 当读0x08000000时调用
    .write = pic_write, // 当写0x08000000时调用
};

```
- 你的代码执行：*(uint32_t*)0x10000000 = 0x01; // 写UART寄存器
- TCG翻译时发现地址在MMIO区域 → 不生成直接内存访问，而是生成辅助函数调用
- 辅助函数调用：memory_region_dispatch_write(addr, value)
- 查表找到pic_ops->write → 执行pic_write()

你的镜像不需要知道QEMU的存在，它只是按RISC-V手册往标准地址写数据。QEMU只是利用页表把那些地址标记为"非RAM"。所以其实，对于一个riscv的架构，其实IO就是写内存映射文件，只不过QUME会对其进行一个转化。将其转移到QEMU模拟的handler中去

#### OpenSBI和ecall机制
OpenSBI是运行在M模式（最高特权级）的固件，你的内核运行在S模式。

特权级层次
```text
U模式（用户程序） ← 你的rCore后续会跑
    ↓ ecall (U->S)
S模式（内核） ← 你的kernel.bin跑在这里
    ↓ ecall (S->M)  
M模式（固件） ← OpenSBI跑在这里
    ↓ 处理实际硬件操作
```
ecall捕获流程
```assembly
# 你的内核代码
ecall  # 从S模式陷入M模式
```
QEMU处理ecall：
```c
case 0b1110011: // ecall指令
    if (cpu->mode == SUPERVISOR) {
        cpu->cause = CAUSE_SUPERVISOR_ECALL;
        cpu->sepc = cpu->pc;
        cpu->scause = CAUSE_SUPERVISOR_ECALL;
        cpu->pc = cpu->stvec;  // 跳转到你的内核trap handler
    } 
    else if (cpu->mode == MACHINE) {
        cpu->cause = CAUSE_MACHINE_ECALL;
        cpu->mepc = cpu->pc;
        cpu->pc = cpu->mtvec;  // 跳转到OpenSBI的入口
    }
```
#### KVM与QUME之间的链接
架构：
```test
┌─────────────────────────────────────┐
│ QEMU (用户态)                        │
│ - 设备模拟 (UART, 网卡, 硬盘)        │
│ - 事件循环                           │
└─────────────┬───────────────────────┘
              │ ioctl(KVM_RUN)
              ▼
┌─────────────────────────────────────┐
│ KVM (内核模块)                       │
│ - 直接执行Guest CPU指令 (硬件虚拟化) │
│ - 处理大部分指令 (无需QEMU参与)      │
│ - 遇到敏感操作才退出到QEMU           │
└─────────────────────────────────────┘
              │
              ▼
          真实CPU
```
本质上QUME因为代表的是硬件的模拟，所以他实际上是在最底层的，但是他本质是运行在用户态的。
KVM是负责vcpu的实际执行的，如果KVM遇到敏感指令，他肯定需要访问硬件，所以遇到敏感指令才会下陷到QEMU。具体的逻辑其实是宿主机一开始是在non-root-mode一旦有了一个敏感指令，就会变成root-mode由kvm接管，kvm拿到控制权在转到qemu
KVM 执行 Guest 时，CPU 遇到敏感指令 → VM-Exit → KVM 拿到控制权

KVM 自己处理不了（比如 MMIO）→ KVM 返回到 QEMU（不是"下陷"）
```text
Guest 正在跑
   ↓ (敏感指令 / 异常 / 中断 / MMIO-> 触发VM-Exit)
CPU 硬件自动保存 Guest 状态，切到 root mode
   ↓
KVM 内核模块拿到控制权
   ↓
KVM 决定：自己处理？还是返回 QEMU？
   ↓
KVM 把 exit_reason 写到共享内存，返回 QEMU（系统调用返回）
   ↓
QEMU 继续执行（设备模拟等）
```
注意一下，EPT这个虚拟机物理地址和宿主机物理地址之间的映射，是由KVM维护的，将虚拟机的物理地址和QEMU链接起来。所以对于QEMU和KVM之间内存的耦合方式就是：宿主机物理内存->KVM的EPT（维护从虚拟机机物理内存到宿主机物理内存）-> 真实物理地址-> QUME中的一段虚拟内存（这里虚拟内存其实就是QUME中的一个数组）
EPT 是 GPA → HPA，HPA 是真实物理地址
```text
Guest 物理地址 (GPA)
    ↓ EPT (由 KVM 维护)
Host 物理地址 (HPA)  ← 这是真实的内存页
    ↓ Host 内核的进程页表
QEMU 进程的虚拟地址 (userspace_addr)  ← 你 mmap 出来的数组
```
KVM本身有两个退出形式一种是不返回QEMU一种是处理不了才返回QEMU。
遇到敏感指令一定是硬件触发VM-Exit让KVM拿到控制权，KVM不会主动去要控制权。

#### QEMU中装载镜像的启动流程

在Qemu模拟的 virt 硬件平台上，物理内存的起始物理地址为 0x80000000 ，物理内存的默认大小为 128MiB ，它可以通过 -m 选项进行配置。如果使用默认配置的 128MiB 物理内存则对应的物理地址区间为 [0x80000000,0x88000000) 。如果使用上面给出的命令启动 Qemu ，那么在 Qemu 开始执行任何指令之前，首先把两个文件加载到 Qemu 的物理内存中：即作把作为 bootloader 的 rustsbi-qemu.bin 加载到物理内存以物理地址 0x80000000 开头的区域上，同时把内核镜像 os.bin 加载到以物理地址 0x80200000 开头的区域上。

为什么加载到这两个位置呢？这与 Qemu 模拟计算机加电启动后的运行流程有关。一般来说，计算机加电之后的启动流程可以分成若干个阶段，每个阶段均由一层软件或 固件 负责，每一层软件或固件的功能是进行它应当承担的初始化工作，并在此之后跳转到下一层软件或固件的入口地址，也就是将计算机的控制权移交给了下一层软件或固件。Qemu 模拟的启动流程则可以分为三个阶段：第一个阶段由固化在 Qemu 内的一小段汇编程序负责；第二个阶段由 bootloader 负责；第三个阶段则由内核镜像负责。

第一阶段：将必要的文件载入到 Qemu 物理内存之后，Qemu CPU 的程序计数器（PC, Program Counter）会被初始化为 0x1000 ，因此 Qemu 实际执行的第一条指令位于物理地址 0x1000 ，接下来它将执行寥寥数条指令并跳转到物理地址 0x80000000 对应的指令处并进入第二阶段。从后面的调试过程可以看出，该地址 0x80000000 被固化在 Qemu 中，作为 Qemu 的使用者，我们在不触及 Qemu 源代码的情况下无法进行更改。

第二阶段：由于 Qemu 的第一阶段固定跳转到 0x80000000 ，我们需要将负责第二阶段的 bootloader rustsbi-qemu.bin 放在以物理地址 0x80000000 开头的物理内存中，这样就能保证 0x80000000 处正好保存 bootloader 的第一条指令。在这一阶段，bootloader 负责对计算机进行一些初始化工作，并跳转到下一阶段软件的入口，在 Qemu 上即可实现将计算机控制权移交给我们的内核镜像 os.bin 。这里需要注意的是，对于不同的 bootloader 而言，下一阶段软件的入口不一定相同，而且获取这一信息的方式和时间点也不同：入口地址可能是一个预先约定好的固定的值，也有可能是在 bootloader 运行期间才动态获取到的值。我们选用的 RustSBI 则是将下一阶段的入口地址预先约定为固定的 0x80200000 ，在 RustSBI 的初始化工作完成之后，它会跳转到该地址并将计算机控制权移交给下一阶段的软件——也即我们的内核镜像。

第三阶段：为了正确地和上一阶段的 RustSBI 对接，我们需要保证内核的第一条指令位于物理地址 0x80200000 处。为此，**我们需要将内核镜像预先加载到 Qemu 物理内存以地址 0x80200000 开头的区域上**。一旦 CPU 开始执行内核的第一条指令，证明计算机的控制权已经被移交给我们的内核，也就达到了本节的目标。

综上跳转的路径为：QEMU的默认0x8000000位置 -> 加载SBI到[0x8000000,0x80200000)上 -> 这个时候跳转到我们要写代码的os部分从0x80200000开始被我们内核接管了。

