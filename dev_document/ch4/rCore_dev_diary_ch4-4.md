# ch4-4 地址空间

这张的实现的代码比较复杂，我们主要说架构和思想

```
┌─────────────────────────────────────────────────────────────┐
│                         MemorySet                           │
│  （一个地址空间 → 每个进程一个）                            	 │
│  ┌────────────────────────┐  ┌────────────────────────┐     │
│  │      PageTable         │  │    areas: Vec<MapArea> │     │
│  │  - root_ppn (根页表物理 │  │   （该空间内的所有连续  	  │     │
│  │    页号)               │  │    虚拟内存区域）        │      │
│  │  - frames (中间页表节点 │  └───────────┬────────────┘      │
│  │    占用的物理帧)        │              │                   │
│  └───────────┬────────────┘             │                   │
│              │                          │                   │
│              │ 页表操作                   ▼                   │
│              │ (map/unmap)         ┌──────────────┐          │
│              │                      │   MapArea    │         │
│              │                      │ - vpn_range  │         │
│              │                      │ - map_type   │         │
│              │                      │ - map_perm   │         │
│              │                      │ - frames (该 │         │
│              │                      │   区域已分    │          │
│              │                      │   配的物理帧)  │         │
│              │                      └──────┬───────┘         │
└──────────────┼─────────────────────────────┼─────────────────┘
               │                             │
               │ 调用 frame_alloc / dealloc   │ 分配物理帧
               ▼                             ▼
          ┌──────────────────────────────────────────┐
          │       物理页帧分配器 (全局唯一)             │
          │   StackFrameAllocator                    │
          │   - current, end (未被分配过的连续区间)     │
          │   - recycled (回收的物理页号栈)            │
          │   alloc()  -> Option<PhysPageNum>       │
          │   dealloc(ppn)                          │
          └──────────────────────────────────────────┘
```

上面是物理内存地址和逻辑内存地址之间的连接关系

假设用户程序访问地址 `0x1000` ，此时该地址所在的虚拟页尚未映射。

1. CPU 触发 Page Fault

- 虚拟地址 `0x1000` 对应虚拟页号 `VPN = 0x1000>>12 = 0x1`。
- MMU 遍历当前页表（`satp` 指向当前进程的 `PageTable`），找不到有效映射，产生 **Load/Store Page Fault**，CPU 跳转到 `stvec` 设置的 trap 入口。

2. trap_handler 分发

- `trap_handler` 读取 `scause`，发现是 Page Fault，调用 `MemorySet::handle_page_fault(vaddr)`（实际代码中可能是 `current_task` 的 `memory_set.handle_page_fault`）。

3. 查找包含该地址的 MapArea

- `MemorySet` 遍历 `areas`，找到第一个覆盖 `vaddr` 的 `MapArea`（通过 `vpn_range` 判断）。
- 若找不到 → 非法访问，杀死进程。

4. 调用 MapArea::map_one(vpn)

- 根据 `map_type`：
  - **`MapType::Framed`**（最常见）：
    1. 调用 `frame_alloc()` 从全局分配器获得一个物理页号 `ppn`，该调用返回 `FrameTracker`（RAII 对象，自动管理释放）。
    2. 将新分配的物理页清零（`FrameTracker::new` 中已做）。
    3. 调用 `PageTable::map(vpn, ppn, flags)` 在页表中建立映射（找到各级页表项并填写）。
    4. 将 `FrameTracker` 存入 `MapArea` 的 `frames` 向量中（以便在 `MapArea` 析构时自动回收）。
  - **`MapType::Identical`**：恒等映射，`ppn = vpn`，不分配新物理页。

5. 返回用户态

- 缺页处理完成，`trap_handler` 返回，执行 `sret`。
- CPU 重新执行之前触发 page fault 的访存指令，此时页表已有映射，访问成功。

----

上面这些过一遍整个虚拟内存的构建流程也就搞定了，虽然里面有很多细节，但是对于底层到底是如何实现了应该是有了比较清晰的认识了



## 内核地址空间

我们的虚拟地址是39位的，但是对于一个64位的系统来说，高位总不能完全没有用，也就是高位会产生一段隔离出来的地址空间，这段地址空间我们不管是用户态还是内核态都会有不同的布局。

- 高部分：

![img](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/kernel-as-high.png)

我们的内核在虚拟地址中的构成是从上往下排列的，每一个部分都是一个虚拟页，最开头的trampoline是一个跳板，后面会学。剩下的段会维护大量的应用内核栈，内核栈之间是有一个守护页去隔离，所以为啥会栈移除就体现在这个位置了。

- 低位

  ![../_images/kernel-as-low.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/kernel-as-low.png)

  内核的四个逻辑段 `.text/.rodata/.data/.bss` 被恒等映射到物理内存，这使得我们在无需调整内核内存布局 `os/src/linker.ld` 的情况下就仍能象启用页表机制之前那样访问内核的各个段。注意我们借用页表机制对这些逻辑段的访问方式做出了限制，这都是为了在硬件的帮助下能够尽可能发现内核中的 bug ，在这里：

  - 四个逻辑段的 U 标志位均未被设置，使得 CPU 只能在处于 S 特权级（或以上）时访问它们；
  - 代码段 `.text` 不允许被修改；
  - 只读数据段 `.rodata` 不允许被修改，也不允许从它上面取指执行；
  - `.data/.bss` 均允许被读写，但是不允许从它上面取指执行。

  ```rust
  extern "C" {
      fn stext();
      fn etext();
      fn srodata();
      fn erodata();
      fn sdata();
      fn edata();
      fn sbss_with_stack();
      fn ebss();
      fn ekernel();
      fn strampoline();
  }
  
  impl MemorySet {
      /// Without kernel stacks.
      pub fn new_kernel() -> Self {
          let mut memory_set = Self::new_bare();
          // map trampoline
          memory_set.map_trampoline();
          // map kernel sections
          println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
          println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
          println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
          println!(".bss [{:#x}, {:#x})", sbss_with_stack as usize, ebss as usize);
          println!("mapping .text section");
          memory_set.push(MapArea::new(
              (stext as usize).into(),
              (etext as usize).into(),
              MapType::Identical,
              MapPermission::R | MapPermission::X,
          ), None);
          println!("mapping .rodata section");
          memory_set.push(MapArea::new(
              (srodata as usize).into(),
              (erodata as usize).into(),
              MapType::Identical,
              MapPermission::R,
          ), None);
          println!("mapping .data section");
          memory_set.push(MapArea::new(
              (sdata as usize).into(),
              (edata as usize).into(),
              MapType::Identical,
              MapPermission::R | MapPermission::W,
          ), None);
          println!("mapping .bss section");
          memory_set.push(MapArea::new(
              (sbss_with_stack as usize).into(),
              (ebss as usize).into(),
              MapType::Identical,
              MapPermission::R | MapPermission::W,
          ), None);
          println!("mapping physical memory");
          memory_set.push(MapArea::new(
              (ekernel as usize).into(),
              MEMORY_END.into(),
              MapType::Identical,
              MapPermission::R | MapPermission::W,
          ), None);
          memory_set
      }
  }
  ```

其实就是从上到下顺下来去操作页面是吧，我们只要指定虚拟页的虚拟地址范围 + 权限 + 映射方式为恒等映射，按照我们内核段的方式组织，这样我们一开始的linker.ld的链接脚本其实也不需要修改。

> 恒等映射：就是将一个虚拟页号和一个物理页号恒等映射咯

## 应用地址空间



我们将起始地址 `BASE_ADDRESS` 设置为 （我们这里并不设置为 ，因为它一般代表空指针），显然它只能是一个地址空间中的虚拟地址而非物理地址。事实上由于我们将入口汇编代码段放在最低的地方，这也是整个应用的入口点。我们只需清楚这一事实即可，而无需像之前一样将其硬编码到代码中。此外，在 `.text` 和 `.rodata` 中间以及 `.rodata` 和 `.data` 中间我们进行了页面对齐，因为前后两个逻辑段的访问方式限制是不同的，由于我们只能以页为单位对这个限制进行设置，因此就只能将下一个逻辑段对齐到下一个页面开始放置。而 `.data` 和 `.bss` 两个逻辑段由于访问限制相同（可读写），它们中间则无需进行页面对齐。

下图展示了应用地址空间的布局：

![../_images/app-as-full.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/app-as-full.png)

这也是讲跳板和TrapContext放到高位剩下的数据布局放到低位这样的内存布局，但是问题最难最难的就是如何去构建这样的一个布局？这就很难了后面的代码我不爱看了太可怕了...

```rust
// os/src/mm/memory_set.rs

impl MemorySet {
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() { map_perm |= MapPermission::R; }
                if ph_flags.is_write() { map_perm |= MapPermission::W; }
                if ph_flags.is_execute() { map_perm |= MapPermission::X; }
                let map_area = MapArea::new(
                    start_va,
                    end_va,
                    MapType::Framed,
                    map_perm,
                );
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize])
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(MapArea::new(
            user_stack_bottom.into(),
            user_stack_top.into(),
            MapType::Framed,
            MapPermission::R | MapPermission::W | MapPermission::U,
        ), None);
        // map TrapContext
        memory_set.push(MapArea::new(
            TRAP_CONTEXT.into(),
            TRAMPOLINE.into(),
            MapType::Framed,
            MapPermission::R | MapPermission::W,
        ), None);
        (memory_set, user_stack_top, elf.header.pt2.entry_point() as usize)
    }
}
```
其实 from_elf 虽然长，但结构非常清晰，分成三段：

- 初始化 MemorySet

- 映射 trampoline

- 循环处理 ELF 的 loadable segments

- 添加用户栈

- 添加 TrapContext 页