use riscv::register::time;

pub fn get_time() -> usize {
    time::read()
}

const MICRO_PER_SEC: usize = 1_000_000;

pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MICRO_PER_SEC)
}