
/*在编译内核之前，扫描 user/src/bin/ 目录下的所有用户程序，生成一个汇编文件 link_app.S，
把每个用户程序的二进制文件（.bin）直接嵌入到内核的数据段中，并导出一个用户程序数量表和每个程序的起始/结束地址，
供内核的批处理调度器使用。 */

// 引入标准库中的文件、目录操作相关类型和 trait
use std::fs::{File, read_dir};
use std::io::{Result, Write};

// build.rs 的主函数，cargo 编译时会先执行
fn main() {
    // 告诉 cargo 如果 ../user/src/ 目录下的文件发生变化，需要重新运行本脚本
    println!("cargo:rerun-if-changed=../user/src/");
    // 同样，如果用户程序的二进制文件目录变化，也要重跑
    println!("cargo:rerun-if-changed={}", TARGET_PATH);
    // 生成 link_app.S 文件，如果失败就 panic
    insert_app_data().unwrap();
}

// 用户程序编译后的二进制文件存放路径（相对于本文件的位置）
static TARGET_PATH: &str = "../user/target/riscv64gc-unknown-none-elf/release/";

// 生成 link_app.S 的核心函数
fn insert_app_data() -> Result<()> {
    // 创建（或覆盖）src/link_app.S 文件
    let mut f = File::create("link_app.S").unwrap();
    // 读取 ../user/src/bin 目录下所有文件名（即用户程序的 rust 源文件）
    let mut apps: Vec<_> = read_dir("../user/src/bin")
        .unwrap()
        .into_iter()
        // 对每个目录项，取出文件名（OsString 转 String）
        .map(|dir_entry| {
            let mut name_with_ext = dir_entry.unwrap().file_name().into_string().unwrap();
            // 删除扩展名（.rs 及其后面的字符），只保留程序名
            name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len());
            name_with_ext
        })
        .collect();
    // 按名字排序，保证每次生成的顺序一致
    apps.sort();

    // 写入汇编代码的头部：.align 3 表示 8 字节对齐；.section .data 放在数据段
    // .global _num_app 导出符号，供内核使用
    // _num_app: 第一个 quad 是应用程序个数
    writeln!(
        f,
        r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {}"#,
        apps.len()
    )?;

    // 写入每个应用程序的起始地址标签（app_0_start, app_1_start, ...）
    for i in 0..apps.len() {
        writeln!(f, r#"    .quad app_{}_start"#, i)?;
    }
    // 写入最后一个应用程序的结束地址标签（用于计算总长度）
    writeln!(f, r#"    .quad app_{}_end"#, apps.len() - 1)?;

    // 对每个应用程序，在数据段嵌入它的二进制文件内容
    for (idx, app) in apps.iter().enumerate() {
        // 打印调试信息（cargo 构建时可见）
        println!("app_{}: {}", idx, app);
        writeln!(
            f,
            r#"
    .section .data
    .global app_{0}_start
    .global app_{0}_end
app_{0}_start:
    .incbin "{2}{1}.bin"
app_{0}_end:"#,
            idx, app, TARGET_PATH
        )?;
    }
    Ok(())
}

