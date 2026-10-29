//! Board benchmark only: DWT measures active handler work, not wall-clock time.
use core::sync::atomic::{AtomicU32, Ordering};
static COUNT: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SUM: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static MAX: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
pub fn irq(index: usize) {
    let start = unsafe { (0xe0001004 as *const u32).read_volatile() };
    embassy_hc32::time_driver::on_interrupt();
    let elapsed = unsafe { (0xe0001004 as *const u32).read_volatile() }.wrapping_sub(start);
    COUNT[index].fetch_add(1, Ordering::Relaxed);
    SUM[index].fetch_add(elapsed, Ordering::Relaxed);
    MAX[index].fetch_max(elapsed, Ordering::Relaxed);
}
pub fn snapshot() -> [[u32; 3]; 2] {
    core::array::from_fn(|i| {
        [
            COUNT[i].load(Ordering::Relaxed),
            SUM[i].load(Ordering::Relaxed),
            MAX[i].load(Ordering::Relaxed),
        ]
    })
}
