# ch4-5 基于地址空间的分时多任务、

## 内核地址空间的建立

 开启分页前：内核直接使用物理地址

- 刚跳转到内核入口（`0x80200000`）时，CPU 在 S 模式，**satp 寄存器为 0**，分页未启用。
- 此时，内核代码中任何访存（如 `ld t0, (a0)`）都会把 `a0` 中的值当作**物理地址**，直接送到内存总线。
- 所以内核最开始能运行，是因为它的链接地址（`0x80200000`）正好对应物理地址 `0x80200000`（QEMU 的物理内存布局把内核放在这个地址）。

 开启分页后：内核只能“看到”虚拟地址

- 当内核初始化页表，并执行 `csrw satp, ...` 启用分页后，CPU 的 MMU 开始工作。
- **从此以后，内核代码中的每一个地址（比如指令 `pc`、数据访问的指针）都会被 MMU 当作虚拟地址**，需要通过页表转换为物理地址才能访问内存。
- 也就是说，内核虽然还在执行**同一段二进制代码**，但代码里的数值（如 `0x80200000`）现在被视为**虚拟地址**。并且在这个时候页表的根地址是已经需要加载到我们的页表基址寄存器里面了。这个地方虚拟地址和物理地址是等价的，因为我们之前设置了恒等映射了。

过渡期间的关键步骤（在 rCore 中）

1. **初始化页表**：`PageTable::new()` 创建一个空页表。
2. **建立恒等映射**：把内核所处的物理内存区域（从 `ekernel` 到 `PHYSICAL_MEMORY_END`）映射到相同的虚拟地址。
   （实际上 rCore 采用更简单的办法：直接让 `MemorySet::new_kernel()` 把所有物理内存恒等映射，包括内核镜像。）
3. **切换页表**：把根页表的物理页号写入 `satp` 寄存器，并刷新 TLB（`sfence.vma`）。
4. **继续执行**：此时内核代码仍在运行，但所有地址都已通过 MMU 转换，且因为恒等映射，功能不受影响。

```rust
pub fn token(&self) -> usize {
    8usize << 60 | self.root_ppn.0
}

// os/src/mm/memory_set.rs

impl MemorySet {
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
    }
}
```

这个activate方法是开启新地图的钥匙，也就是调用了memset的activate方法之后正式启动分页

构造 Token (钥匙)：

- `page_table.token()`：把根页表的**物理页号 (PPN)** 和 **Sv39 模式标识 (8)** 拼成一个 64 位整数。
- 这个整数就是打开新地图的钥匙。

token这个概念我们后面会经常在切换地址的时候使用，后面有一个很细节的平滑过渡的概念：

> **指令 N**：`csrw satp, ...` (写入寄存器)。
>
> **指令 N+1**：`sfence.vma` (下一条指令)。
>
> **问题**：指令 N 执行时，MMU 还没开（或用的是旧配置）；指令 N+1 执行时，MMU 已经开了，且用的是**新页表**。
>
> 如何保证不崩？
>
>  这就是文中提到的**“平滑过渡”**。
>
> - 因为我们在建立页表时，确保了**内核代码段**在“旧视角”（物理地址直连）和“新视角”（内核虚拟地址）下，指向的是**同一块物理内存**。
> - 所以，无论 MMU 查哪张表，取到的指令 N+1 的物理地址都是一样的。CPU 顺利拿到了下一条指令，没有因为“查表失败”而崩溃。

## 地址空间之间的跳板

跳板在前一份笔记里面讲过，内核还是应用的地址空间，最高的虚拟页面都是一个跳板。同时应用地址空间的次高虚拟页面还被设置为用来存放应用的 Trap 上下文。跳板对于内核还是地址，虽然地址空间不同，但是指向的物理页面（代码）是一样的，所有应用和内核跳板的相对位置也是固定的。

开启分页机制之后，我们必须在这个过程中同时完成地址空间的切换。具体来说，当 `__alltraps` 保存 Trap 上下文的时候，我们必须通过修改 satp 从应用地址空间切换到内核地址空间，因为 trap handler 只有在内核地址空间中才能访问；同理，在 `__restore` 恢复 Trap 上下文的时候，我们也必须从内核地址空间切换回应用地址空间

> **内核与应用地址空间的隔离**
>
> 目前我们的设计思路 A 是：对内核建立唯一的内核地址空间存放内核的代码、数据，同时对于每个应用维护一个它们自己的用户地址空间，因此在 Trap 的时候就需要进行地址空间切换，而在任务切换的时候无需进行（因为这个过程全程在内核内完成）。
>
> 另外的一种设计思路 B 是：让每个应用都有一个包含应用和内核的地址空间，并将其中的逻辑段分为内核和用户两部分，分别映射到内核/用户的数据和代码，且分别在 CPU 处于 S/U 特权级时访问。此设计中并不存在一个单独的内核地址空间。
>
> 设计方式 B 的优点在于： Trap 的时候无需切换地址空间，而在任务切换的时候才需要切换地址空间。相对而言，设计方式B比设计方式A更容易实现，在应用高频进行系统调用的时候，采用设计方式B能够避免频繁地址空间切换的开销，这通常源于快表或 cache 的失效问题。但是设计方式B也有缺点：即内核的逻辑段需要在每个应用的地址空间内都映射一次，这会带来一些无法忽略的内存占用开销，并显著限制了嵌入式平台（如我们所采用的 K210 ）的任务并发数。此外，设计方式 B 无法防御针对处理器电路设计缺陷的侧信道攻击（如 [熔断 (Meltdown) 漏洞](https://cacm.acm.org/magazines/2020/6/245161-meltdown/fulltext) ），使得恶意应用能够以某种方式间接“看到”内核地址空间中的数据，使得用户隐私数据有可能被泄露。将内核与地址空间隔离便是修复此漏洞的一种方法。

这种隔离方式可以积累一下，不同地址空间的布局也可以稍微了解一下。

其实跳板的核心作用是，用户态和内核态都可以找到跳板，这个跳板是一块内存空间，也就是用这个方式将内核态和用户态链接起来了。

### 跳板的组成

#### 1. 入口代码段：`__alltraps` (The Entry)

这是 Trap 发生时 CPU 跳进来的第一站。

- **位置**：通常对齐到页的起始地址（Offset 0）。
- 职责：
  - **保存现场**：把用户态的所有通用寄存器（x1-x31）压入栈中。
  - **准备参数**：为调用 Rust 写的 `trap_handler` 做准备。
  - **切换页表**：从跳板数据区读取内核页表基地址，写入 `satp`，执行 `sfence.vma`。（这个页表基址是内核在启动的时候填充进去的）
  - **跳转**：跳转到内核深处的 `trap_handler`。

#### 2. 出口代码段：`__restore` (The Exit)

这是从内核返回用户态时的最后一站。

- **位置**：紧跟在 `__alltraps` 之后（或者在页内的某个固定偏移处）。
- 职责：
  - **接收参数**：接收来自内核的两个关键参数——用户页表基地址（Token）和用户 Trap 上下文的虚拟地址。
  - **切换页表**：写入用户 Token，刷新 TLB，瞬间回到用户态视角。（**所以这就是为啥后续要改TaskContext，需要将token也记录下来便于回复用户状态**）
  - **恢复现场**：从用户栈中弹出之前保存的寄存器。
  - **返回**：执行 `sret`，让 CPU 变回用户态，继续执行用户程序。

#### 3. 数据区：共享状态存储 (The Data Bridge)

**这部分最容易被忽略，但它是跳板能工作的灵魂！** 因为代码和数据在同一个物理页里，而这一页同时映射在内核和用户空间，所以这里成了**两个世界交换信息的“秘密信箱”**。这部分的数据是应用和内核注入的

通常包含以下几个关键字段（在 rCore 实现中）：

- `kernel_satp`

  ：内核页表的根节点物理地址。

  - *谁写？* 内核初始化进程时写入。
  - *谁读？* `__alltraps` 在进入内核时读取，用来切换地图。

- `trap_handler_addr`：trap_handler 函数的内核虚拟地址。

  - *谁写？* 内核初始化时写入。
  - *谁读？* `__alltraps` 读取后，通过 `jr` 跳过去。

- `user_sp` / `trap_cx_ptr`

  ：用户栈顶或 Trap 上下文地址。

  - *作用*：告诉汇编代码，用户的寄存器快照存在哪。

> **💡 为什么需要这个数据区？** 因为 `__alltraps` 执行时，CPU 还在用**用户页表**。它看不见内核全局变量（比如 `KERNEL_SPACE`）。它只能看见**当前页**里的东西。所以，我们必须把内核的关键信息“拷贝”到这个共享页里，`__alltraps` 才能拿到它们。

#### 4. 填充与对齐：Padding & Alignment

- **位置**：页的剩余部分。
- 职责：
  - 确保 `__alltraps` 严格对齐到页首（方便计算偏移）。
  - 确保整个结构体大小正好等于一个物理页（4KB）。
  - 防止指令跨越页边界，导致取指错误。

> 在内存级别的编程中，页面填充和对齐是我之前没有接触过的了

```asm
# os/src/trap/trap.S

    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2
__alltraps:
    csrrw sp, sscratch, sp # 将sscratch寄存器中的值和sp中的值交换一下
    # now sp->*TrapContext in user space, sscratch->user stack
    # save other general purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    # we can use t0/t1/t2 freely, because they have been saved in TrapContext
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # read user stack from sscratch and save it in TrapContext
    # 内核会把内核的页表基地址 (kernel_satp)、内核栈指针 (kernel_sp)、内核处理函数地址 (trap_handler) 这三个值，预先写入到这个 TrapContext 结构体的特定偏移处（也就是 34*8, 35*8, 36*8 的位置）。
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # load kernel_satp into t0
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # move to kernel_sp
    ld sp, 35*8(sp)
    # switch to kernel space
    csrw satp, t0
    sfence.vma
    # jump to trap_handler
    jr t1

__restore:
    # a0: *TrapContext in user space(Constant); a1: user space token
    # switch to user space
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0
    mv sp, a0
    # now sp points to TrapContext in user space, start restoring based on it
    # restore sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general purpose registers except x0/sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)
    sret
```

> 用jr跳转而不是利用call命令跳转的原因是:
>
> **跳转指令实际被执行时的虚拟地址和在编译器/汇编器/链接器进行后端代码生成和链接形成最终机器码时设置此指令的地址是不同的。**所以如果用相对寻址会报错因为他已经不在这个相对的位置了,在很远的地方没法简单通过地址偏移去进行访问了.

## 内核任务控制块的修改

我们的任务控制和trap的很多东西都要改,因为原来的控制逻辑并没有区分地址空间,现在一旦进行了进程context之间的切换,那么就一定要设计地址空间上下文的切换了

为了管理隔离的地址空间，`TaskControlBlock` 增加了以下关键成员：

| 字段          | 类型          | 作用                                                         |
| :------------ | :------------ | :----------------------------------------------------------- |
| `memory_set`  | `MemorySet`   | 应用的完整地址空间（包含代码、数据、栈、跳板映射）。         |
| `trap_cx_ppn` | `PhysPageNum` | **Trap 上下文所在的物理页号**。用于内核在不切换页表的情况下，直接定位并修改用户空间的 Trap 数据。 |
| `base_size`   | `usize`       | 应用静态数据的大小（用于后续堆扩展）。                       |
| `task_cx`     | `TaskContext` | 任务切换上下文，首次启动时指向 `trap_return`。               |

```rust
/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

// os/src/task/task.rs

impl TaskControlBlock {
    // 其实看这个new函数,可以发现我们只是一直往里面去填入东西是吧
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // 解析 ELF：调用 MemorySet::from_elf 建立应用地址空间，获取入口点 entry_point 和用户栈顶 user_sp。
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 定位 Trap 上下文：通过页表查询，找到用户空间中 TRAP_CONTEXT 对应的物理页号 trap_cx_ppn。
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // 分配内核栈：在内核地址空间中为每个应用分配独立的内核栈（防止应用崩溃污染内核或其他应用）。
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
            .exclusive_access()
            .insert_framed_area(
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            );
        // 初始化 Trap 上下文
//利用 trap_cx_ppn 获得可变引用。
//填入关键信息：sepc (入口点), kernel_satp (内核页表), kernel_sp (内核栈), trap_handler (处理函数地址)。
//目的：让 __alltraps 在进入内核后能顺利找到回家的路。
        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
}
```

## 改进的 Trap 处理链路 (核心难点)

#### 进入内核 (`__alltraps`)

- **交换栈指针**：利用 `sscratch` 将用户 `sp` 换出，使 `sp` 指向 `TrapContext`。
- **保存现场**：将所有通用寄存器及 `sstatus/sepc` 存入 `TrapContext`。
- **读取内核门票**：从 `TrapContext` 尾部读取预存的 `kernel_satp` 和 `trap_handler`。
- 切换与跳转：
  1. `csrw satp, kernel_satp` (切换到内核地图)。
  2. `sfence.vma` (刷新 TLB)。
  3. `jr trap_handler` (绝对跳转至内核 Rust 代码)。

#### 内核处理 (`trap_handler`)

- **设置防崩机制**：调用 `set_kernel_trap_entry`，若内核再发生 Trap 则直接 Panic。
- **获取上下文**：通过 `current_trap_cx()` 拿到当前应用 Trap 上下文的引用。
- **分发处理**：根据 `scause` 处理系统调用、缺页异常或时钟中断。

#### 返回用户态 (`trap_return`)

- **重置入口**：调用 `set_user_trap_entry`，将 `stvec` 改回跳板页地址 `TRAMPOLINE`。

- 计算恢复地址：

  - `restore_va = TRAMPOLINE + (__restore - __alltraps)`。
  - **原理**：利用编译期确定的偏移量，算出 `__restore` 在用户态虚拟地址空间的位置。

- 执行汇编跳转

  ：

  1. `fence.i` (清空指令缓存，防止跨进程代码污染)。
  2. `jr restore_va` (跳转到用户态的 `__restore` 代码)。

#### 恢复现场 (`__restore`)

- **切换回用户页表**：`csrw satp, user_token`。
- **恢复寄存器**：从 `TrapContext` 弹出所有寄存器值。
- **返回**：执行 `sret`，CPU 回到用户态继续执行。

## 跨空间数据访问：以 `sys_write` 为例

由于地址空间隔离，内核不能直接解引用用户指针 `buf`。

### 解决方案：`translated_byte_buffer`

1. **输入**：用户 Token、用户虚拟地址 `ptr`、长度 `len`。
2. **查表翻译**：遍历用户页表，将连续的虚拟地址范围拆解为若干个**物理页帧 (PPN)**。
3. **内核映射**：通过 PPN 找到内核空间中对应的字节切片 (`&[u8]`)。
4. **返回向量**：返回一组内核可直接访问的切片向量（处理了跨页边界的情况）。

