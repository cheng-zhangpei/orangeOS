# ch4-3 管理SV39页表

## 分配页框

对于一个物理内存来说，其中其实已经有了一部分的空间分配了os的内核空间，然后从内核空间结束的部分开始到我们的memory_end的地址8KIB的空间，是剩余的空闲页。

```rust
pub struct StackFrameAllocator {
    current: usize,      // 空闲内存区间的起始物理页号（从未分配过的）
    end: usize,          // 空闲内存区间的结束物理页号（不包含）
    recycled: Vec<usize>,// 被回收的物理页号栈（后进先出）
}
fn alloc(&mut self) -> Option<PhysPageNum> {
    if let Some(ppn) = self.recycled.pop() {
        Some(ppn.into())         // 1. 优先用回收的页号
    } else {
        if self.current == self.end {
            None                 // 2. 无空闲页
        } else {
            self.current += 1;   // 3. 从未分配区间取左端
            Some((self.current - 1).into())
        }
    }
}
```

其实看这个代码，其实发现这个内存分配器本质其实就是对页号进行管理咯。这个栈式分配器思维其实不难，分配的过程其实就是，如果前面有页被释放了，就直接从栈里面拿到这个页，如果没有空闲页就做页替换（这里没有替换），如果没满就自然往下分配就好了

```rust
fn dealloc(&mut self, ppn: PhysPageNum) {
    let ppn = ppn.0;
    // 检查是否已被分配：ppn < self.current（从未分配区间的左边界）
    if ppn >= self.current || self.recycled.iter().find(|&v| *v == ppn).is_some() {
        panic!("Frame ppn={:#x} has not been allocated!", ppn);
    }
    self.recycled.push(ppn);
}
```

- 回收前做**合法性校验**：
  1. 页号必须小于 `current`（否则它从未被分配过）。
  2. 页号不能在 `recycled` 栈中（否则是重复回收）。
- 校验通过后压入回收栈。

---

## RAII**（Resource Acquisition Is Initialization）**

教程中，alloc物理页面的时候会同步出现一个FrameTracker

**`FrameTracker` 就是在分配物理页时创建一个“票据”，当这个票据被销毁时，自动把物理页归还给分配器。**

这就是 **RAII（Resource Acquisition Is Initialization）** 的核心思想。它的意义不在于“多封装一层”，而在于**利用编程语言的自动析构机制，把资源的生命周期与对象的生命周期绑定**，从而彻底避免手动管理资源带来的各种错误。

假设没有 `FrameTracker`，你分配一个物理页后会得到一个 `PhysPageNum`，然后在不用时必须手动调用 `frame_dealloc(ppn)`。那么你可能会犯以下错误：

- **忘记释放** → 内存泄漏。
- **重复释放**（double free） → 分配器状态损坏，后续分配可能出错。
- **释放后继续使用**（use-after-free） → 可能读到脏数据或破坏其他模块的数据。
- **提前释放** → 导致悬垂指针，同样危险。

`FrameTracker` 做了三件事：

1. **构造时获取资源**（分配物理页，并清零）。
2. **提供资源的访问接口**（通过 `ppn` 字段或方法）。
3. **析构时释放资源**（实现 `Drop` trait，自动调用 `frame_dealloc`）。

Rust 保证：**当一个 `FrameTracker` 对象离开作用域时，它的 `drop` 方法一定会被调用**（除非你刻意用 `std::mem::forget` 或泄漏它）。于是：

- **不会忘记释放**：因为对象销毁是自动的，你无法“忘记”。
- **不会重复释放**：因为每个物理页只被包装在一个活的 `FrameTracker` 中（所有权转移），释放只发生一次。
- **不会释放后使用**：因为一旦对象被销毁，你就无法再访问它的 `ppn`（除非你提前把 `ppn` 复制出来并忘记它——但那是你自己的选择，编译器不会强制）。

另外，在 `FrameTracker::new` 中清零物理页，保证了每次分配的页面都是干净的，避免信息泄漏。ok说人话，就是方便编程，方便管理

----

RAII 是 Rust （以及 C++）中管理**任何资源**（内存、文件句柄、锁、网络连接等）的标准模式。它的核心理念是：

> **资源的生命周期应该与拥有它的对象的生命周期完全一致。**

这样，程序员只需要关心对象的创建和销毁时机（通过作用域、所有权转移等），而资源的具体释放细节由析构函数自动完成。

在大型系统中，这种模式极大地降低了心智负担，让资源管理变得“理所当然”，从而减少 bug。

最后做一个小结：从其他内核模块的视角看来，物理页帧分配的接口是调用 `frame_alloc` 函数得到一个 `FrameTracker` （如果物理内存还有剩余），它就代表了一个物理页帧，当它的生命周期结束之后它所控制的物理页帧将被自动回收。

## 多级页表管理

```rust
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
}
```

每个应用的地址空间都对应一个不同的多级页表，这也就意味这不同页表的起始地址（即页表根节点的地址）是不一样的。因此 `PageTable` 要保存它根节点的物理页号 `root_ppn` 作为页表唯一的区分标志。此外，向量 `frames` 以 `FrameTracker` 的形式保存了页表所有的节点（包括根节点）所在的物理页帧。这与物理页帧管理模块的测试程序是一个思路，即将这些 `FrameTracker` 的生命周期进一步绑定到 `PageTable` 下面。当 `PageTable` 生命周期结束后，向量 `frames` 里面的那些 `FrameTracker` 也会被回收，也就意味着存放多级页表节点的那些物理页帧被回收了。

当我们通过 `new` 方法新建一个 `PageTable` 的时候，它只需有一个根节点。为此我们需要分配一个物理页帧 `FrameTracker` 并挂在向量 `frames` 下，然后更新根节点的物理页号 `root_ppn` 。

多级页表并不是被创建出来之后就不再变化的，为了 MMU 能够通过地址转换正确找到应用地址空间中的数据实际被内核放在内存中位置，操作系统需要动态维护一个虚拟页号到页表项的映射，支持插入/删除键值对，其方法签名如下：

```rust
// os/src/mm/page_table.rs
// 所以的确啊，页表这个玩意儿本质不就是一个键值系统吗？
impl PageTable {
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags);
    pub fn unmap(&mut self, vpn: VirtPageNum);
}
```

具体的map和unmap本质是操作我们的root Page Num的位置的物理页中的内容，一项一项的去建立映射

-----

**开启分页后，CPU 只看虚拟地址，不认物理地址。**
如果内核想访问某个物理地址 `pa`，就**在页表里加一条映射**：让某个虚拟地址 `va` 指向 `pa`。然后内核访问 `va`，MMU 会自动把它转成 `pa`。
最简单的办法是 **恒等映射**：让 `va == pa`，这样你按物理地址写的代码在分页模式下也能正常工作。

## 内核中如何访问一个特定的物理帧

```rust
impl VirtPageNum {
    /// 将虚拟页号分解为三级页表索引（Sv39）
    /// 返回数组 [索引2, 索引1, 索引0]，分别对应三级、二级、一级页表的偏移
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;          // 虚拟页号（39位有效）
        let mut idx = [0usize; 3];
        // 从最低级（一级）索引开始取，但存入数组时高位在前
        for i in (0..3).rev() {
            idx[i] = vpn & 511;        // 取低9位（每级页表512项）
            vpn >>= 9;                 // 右移9位，处理下一级
        }
        idx
    }
}

impl PageTable {
    /// 查找虚拟页号对应的页表项（可变引用），若路径上的中间页表缺失则自动分配
    /// 返回页表项的可变引用，如果中间页表无法分配（内存耗尽）则可能 panic
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();               // 三级索引 [level2, level1, level0]
        let mut ppn = self.root_ppn;            // 当前节点的物理页号，初始为根页表物理页号
        let mut result: Option<&mut PageTableEntry> = None;

        for i in 0..3 {                        // 遍历三级：i=0 对应最高级（第二级），i=2 对应叶子级（第零级）
            // 获取当前页表节点中第 idxs[i] 个页表项的可变引用
            let pte = &mut ppn.get_pte_array()[idxs[i]];

            if i == 2 {                        // 已到达叶子级（第零级），直接返回该页表项
                result = Some(pte);
                break;
            }

            // 中间级节点：检查页表项是否有效（是否指向下一级页表）
            if !pte.is_valid() {
                // 无效：分配一个新物理页帧作为下一级页表
                let frame = frame_alloc().unwrap();
                // 设置当前页表项指向新帧，并标记为有效
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // 保存该帧的 FrameTracker，以便在 PageTable 销毁时自动回收
                self.frames.push(frame);
            }
            // 继续向下一级遍历：更新 ppn 为下一级页表的物理页号
            ppn = pte.ppn();
        }

        result
    }

    /// 查找虚拟页号对应的页表项（可变引用），如果路径上的中间页表缺失则直接返回 None
    /// 不会修改页表结构，也不会分配新页表
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;

        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i == 2 {
                result = Some(pte);
                break;
            }
            // 中间级：如果页表项无效，说明下一级不存在，直接返回 None
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }

        result
    }
}
```

实话我有点不想看代码了，但是大概的意思就是创建一个页表项、和查找一个页表项的算法。这就是一个根据当前的ppn开始一级一级往下递归找到叶节点。

```rust
// os/src/mm/page_table.rs

impl PageTable {
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
}
```

这样看这两个函数好像是看得懂。这里面还有一个隐含的映射就是页号到我们实际的物理地址的映射。



