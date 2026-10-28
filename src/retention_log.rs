use core::fmt::Write;
struct Logger;
static LOGGER: Logger = Logger;
impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        if matches!(record.args().as_str(), Some("irq" | "SETUP waiting")) {
            return;
        }
        critical(record);
    }
    fn flush(&self) {}
}
fn critical(record: &log::Record<'_>) {
    cortex_m::interrupt::free(|_| {
        struct Writer;
        impl Write for Writer {
            fn write_str(&mut self, text: &str) -> core::fmt::Result {
                unsafe {
                    let index = 0x200f0c00 as *mut u32;
                    let mut cursor = index.read_volatile();
                    for byte in text.bytes() {
                        (0x200f0000 as *mut u8)
                            .add(cursor as usize % 3072)
                            .write_volatile(byte);
                        cursor = cursor.wrapping_add(1);
                    }
                    index.write_volatile(cursor);
                }
                Ok(())
            }
        }
        let _ = writeln!(Writer, "{}", record.args());
    });
}
pub fn init() {
    unsafe {
        (0x200f0c00 as *mut u32).write_volatile(0);
    }
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
