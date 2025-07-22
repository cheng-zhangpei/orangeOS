# os/src/trap/trap.S
.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm

.align 2
# 总结一下，这段内容是用户主动trap的前置准备，也就是保存用户的上下文。一共保存下面三个内容
# 1. x1~x31 的通用寄存器
# 2. t1、t2寄存器用于保存CSR中的sstatus、sepc
# 3. 用户栈的指针

__alltraps:
    csrrw sp, sscratch, sp
    # now sp->kernel stack, sscratch->user stack，在这一行之前 sp 指向用户栈， sscratch 指向内核栈（原因稍后说明）
    # allocate a TrapContext on kernel stack
    addi sp, sp, -34*8
    #=============================保存通用寄存器================================
    sd x1, 1*8(sp) # 这个意思是偏移8个字节的意思，因为riscv是64bit架构，一个寄存器是64位的
    # skip sp(x2), we will save it later，所以中间空出来了一个寄存器
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    #====================保存状态寄存器中的两个寄存器用于trap的恢复===============
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # =======================读取用户栈指针并将其保存在内核栈====================
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # set input argument of trap_handler(cx: &mut TrapContext)
    mv a0, sp
    # =======================调用对应的trap_handler处理用户触发内陷====================
    call trap_handler



.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
# 很显然这个函数就是用于恢复用户程序的上下文的
__restore:
    # case1: start running app by __restore
    # case2: back to U after handling trap
    mv sp, a0
    # =================================把trap的三个状态寄存器从栈里面拿出来=====================
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # ==================================恢复通用===================================
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # release TrapContext on kernel stack
    addi sp, sp, 34*8
    # ================================让sp指向用户栈，sscratch指向内核栈=====================
    csrrw sp, sscratch, sp
    # =====================================切换特权级================================
    sret