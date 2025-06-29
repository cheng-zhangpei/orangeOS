use crate::sbi::shutdown;
use core::panic::PanicInfo;
use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(message) = info.message().as_str() {
        if let Some(location) = info.location() {
            println!("Panicked at {}:{} {}", location.file(), location.line(), message);
        } else {
            println!("Panicked: {}", message);
        }
    } else {
        println!("Unknown panic");
    }

    shutdown(true); // 这个函数必须返回 `!`！
}

