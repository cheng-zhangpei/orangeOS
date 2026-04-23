
 pub fn load_apps() {
     extern "C" { fn _num_app(); }
     let num_app_ptr = _num_app as usize as *const usize;
     let num_app = get_num_app();
     let app_start = unsafe {
         core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1)
     };
     // load apps
     for i in 0..num_app {
         let base_i = get_base_i(i);
         // clear region => 将我们的应用程序的内存区域清零
         (base_i..base_i + APP_SIZE_LIMIT).for_each(|addr| unsafe {
             (addr as *mut u8).write_volatile(0)
         });
         // load app from data section to memory
         let src = unsafe {
             core::slice::from_raw_parts(
                 app_start[i] as *const u8, // 起始位置 
                 app_start[i + 1] - app_start[i] // 大小
             )
         };
         let dst = unsafe {
             core::slice::from_raw_parts_mut(base_i as *mut u8, src.len())
         };
         dst.copy_from_slice(src);
     }
     unsafe {
         asm!("fence.i");
     }
 }
  fn get_base_i(app_id: usize) -> usize {
     APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
 }
// 这个函数的详细内容如下，我系统启动的时候，是默认处于S态的，但是我要执行用户程序需要在用户态才行，所以这里就需要向
 pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_context(
        TrapContext::app_init_context(get_base_i(app_id), USER_STACK[app_id].get_sp()),
    )
}