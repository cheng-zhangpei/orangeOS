   .section .text.entry #将后面的内容全部放到一个名为.text.entry 的段中
   .globl _start #符号 _start 的地址即为第 5 行的指令所在的地址。第 3 行我们告知编译器 _start 是一个全局符号，因此可以被其他目标文件使用
_start:
     la sp, boot_stack_top
        call rust_main
        .section .bss.stack # 后面的内容放到.bss.stack段中
        .globl boot_stack_lower_bound
    boot_stack_lower_bound:
        .space 4096 * 16
        .globl boot_stack_top
    boot_stack_top: