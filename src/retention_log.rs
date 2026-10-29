//! Retained text trace. Formatting must never mask PD capture interrupts.
use core::fmt::Write;
struct Logger;
static LOGGER: Logger = Logger;
const CAPACITY: usize = 3072;
fn cursor() -> *mut u32 {
    0x200f0c00 as *mut u32
}
impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        // The software GoodCRC path must not format verbose stack messages
        // before starting TX. Packet diagnostics are queued by the board driver.
        if record.target().starts_with("usbpd") && record.level() > log::Level::Warn {
            return;
        }
        if matches!(record.args().as_str(), Some("irq" | "SETUP waiting")) {
            return;
        }
        // First count while interrupts remain enabled. Reserve a contiguous logical
        // span in one short critical section, then format into it outside that lock.
        struct Count(usize);
        impl Write for Count {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                self.0 = self.0.saturating_add(s.len());
                Ok(())
            }
        }
        let mut count = Count(0);
        let _ = writeln!(count, "{}", record.args());
        let length = count.0.min(CAPACITY);
        let start = cortex_m::interrupt::free(|_| unsafe {
            let n = cursor().read_volatile();
            cursor().write_volatile(n.wrapping_add(length as u32));
            n
        });
        struct Reserved {
            position: u32,
            remaining: usize,
            line: heapless::String<192>,
        }
        impl Write for Reserved {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let _ = self.line.push_str(s);
                for byte in s.bytes() {
                    if self.remaining == 0 {
                        break;
                    }
                    // Bound interrupt masking to cursor check + one byte. If a nested writer
                    // has wrapped the ring meanwhile, never overwrite its newer data with an
                    // older reserved record. Reset/panic may still leave an incomplete record.
                    cortex_m::interrupt::free(|_| unsafe {
                        if cursor().read_volatile().wrapping_sub(self.position) <= CAPACITY as u32 {
                            (0x200f0000 as *mut u8)
                                .add(self.position as usize % CAPACITY)
                                .write_volatile(byte);
                        }
                    });
                    self.position = self.position.wrapping_add(1);
                    self.remaining -= 1;
                }
                Ok(())
            }
        }
        let mut reserved = Reserved {
            position: start,
            remaining: length,
            line: heapless::String::new(),
        };
        let _ = writeln!(reserved, "{}", record.args());
        if record.level() <= log::Level::Info {
            crate::rpc::log(reserved.line);
        }
    }
    fn flush(&self) {}
}
pub fn init() {
    unsafe {
        cursor().write_volatile(0);
    }
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
