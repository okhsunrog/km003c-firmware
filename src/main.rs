#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use cortex_m_rt::{entry, exception};

mod usb;
const MARKER: *mut u32 = 0x200f_0fc0 as *mut u32;
const BOOT_REQUEST: *mut u32 = 0x200f_0ff0 as *mut u32;

const fn metadata() -> [u8; 64] {
    let mut data = [0; 64];
    data[1] = 0x40;
    let mut i = 8;
    while i < 16 {
        data[i] = 0xff;
        i += 1;
    }
    let fields: [&[u8]; 3] = [b"KM003C", b"rust-smoke", b"2026.10.28"];
    let mut field = 0;
    while field < 3 {
        i = 0;
        while i < fields[field].len() {
            data[16 + field * 12 + i] = fields[field][i];
            i += 1;
        }
        field += 1;
    }
    data
}

#[used]
#[unsafe(link_section = ".km_metadata")]
static METADATA: [u8; 64] = metadata();

// The loader jumps without resetting the MCU. Mask inherited interrupts before
// cortex-m-rt initializes RAM and application state.
core::arch::global_asm!(
    ".section .text.__pre_init,\"ax\"",
    ".global __pre_init",
    ".thumb_func",
    "__pre_init:",
    "cpsid i",
    "ldr r0, =0xe000e010",
    "movs r1, #0",
    "str r1, [r0]",
    "ldr r0, =0xe000e180",
    "ldr r2, =0xe000e280",
    "movs r1, #0",
    "mvns r1, r1",
    "movs r3, #5",
    "2:",
    "str r1, [r0], #4",
    "str r1, [r2], #4",
    "subs r3, #1",
    "bne 2b",
    "dsb",
    "isb",
    "bx lr",
);

fn return_to_loader(status: u32) -> ! {
    unsafe {
        write_volatile(MARKER.add(3), status);
        write_volatile(BOOT_REQUEST, 0x1234_5678);
    }
    cortex_m::asm::dsb();
    cortex_m::peripheral::SCB::sys_reset()
}

#[entry]
fn main() -> ! {
    let _device = embassy_hc32::pac::Peripherals::take().unwrap();
    unsafe {
        write_volatile(MARKER, 0x5255_5354);
        write_volatile(MARKER.add(1), 1);
        write_volatile(MARKER.add(2), 0x0000_4000);
    }
    // Cycle delay only: leave all inherited clocks, GPIO and analog state alone.
    cortex_m::asm::delay(16_000_000);
    return_to_loader(0x600d_0001)
}

#[exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    return_to_loader(0xbad0_0001)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    return_to_loader(0xbad0_0002)
}
