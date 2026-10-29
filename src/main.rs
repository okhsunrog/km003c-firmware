#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use cortex_m_rt::{entry, exception};
use embassy_hc32::pac::interrupt;

mod aw9523;
mod cc_frontend;
mod display;
mod lines;
pub mod measurement;
mod orientation;
mod pd_buffered;
mod pd_capture;
mod pd_fast;
mod pd_session;
mod pd_sink;
mod pd_tx;
mod retention_log;
mod rpc;
mod rpc_pd;

mod ui;
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
    let fields: [&[u8]; 3] = [b"KM003C", b"rust-rpc", b"2026.10.28"];
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
    // Disabling SysTick does not clear an already pending exception. Clear
    // inherited SysTick/PendSV before enabling interrupts in the application.
    "ldr r0, =0xe000ed04",
    "ldr r1, =0x0a000000",
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
    cortex_m::interrupt::disable();
    pd_tx::release();
    unsafe {
        write_volatile(MARKER.add(3), status);
        write_volatile(BOOT_REQUEST, 0x1234_5678);
    }
    cortex_m::asm::dsb();
    cortex_m::peripheral::SCB::sys_reset()
}

/// Normal recovery can await the external expander before resetting the MCU.
/// Fault handlers cannot run I2C; they still stop the direct TX hardware above.
async fn enter_loader(status: u32) -> Result<(), &'static str> {
    pd_tx::release_and_wait().await?;
    return_to_loader(status)
}

#[entry]
fn main() -> ! {
    retention_log::init();
    let _device = embassy_hc32::pac::Peripherals::take().unwrap();
    unsafe {
        for i in 0..12 {
            write_volatile(MARKER.add(i), 0);
        }
        write_volatile(MARKER, 0x5255_5354);
        write_volatile(MARKER.add(1), 1);
        write_volatile(MARKER.add(2), 0x0000_4000);
    }
    use embassy_hc32::clocks::{self, Config, Divider, Dividers, Mpll};
    unsafe {
        write_volatile(0x40050804 as *mut u32, 0x77);
        // DDL 3.3.0 BSP_CLK_Init at 200 MHz: SRAMH=0 waits, SRAM12/3/R=1.
        write_volatile(0x40050800 as *mut u32, 0x11001111);
        write_volatile(0x40050804 as *mut u32, 0x76);
    }
    let clocks = unsafe {
        clocks::configure(Config {
            mpll: Mpll::new(2, 48, [2, 4, 8]).unwrap(),
            dividers: Dividers {
                hclk: Divider::Div1,
                exclk: Divider::Div4,
                pclk: [
                    Divider::Div2,
                    Divider::Div2,
                    Divider::Div4,
                    Divider::Div4,
                    Divider::Div2,
                ],
            },
        })
        .unwrap()
    };
    let hclk = clocks.hclk;
    assert_eq!(hclk, 192_000_000);
    log::info!("Clock readback: {clocks:?}");
    pd_sink::init();
    // No timer pin/output is enabled. PCLK1 is fixed after startup.
    unsafe {
        // Application startup exclusively owns all routes. A loader jump can
        // leave selectors mapped even though pre-init disabled their NVICs.
        embassy_hc32::intc::clear_routes();
        embassy_hc32::time_driver::init(
            _device.tmra1,
            clocks.pclk[1],
            embassy_hc32::intc::Line::new(80).unwrap(),
            embassy_hc32::intc::Line::new(81).unwrap(),
            5,
        )
        .unwrap();
    }
    let (aw, _aw_routes) = unsafe { aw9523::Aw9523::new() };
    // USB core/PHY startup still has synchronous settling delays. Finish those
    // before polling any async I2C operation: otherwise a pending AW transfer
    // can spend its entire timeout waiting for this executor to resume.
    unsafe { write_volatile(MARKER.add(4), 1) };
    static USB_OUT: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    let usb_start = embassy_time::Instant::now();
    let (usb_driver, usb_route) = unsafe { usb::driver(USB_OUT.init([0; 256])) };
    log::info!(
        "USB hardware startup: {} ms",
        usb_start.elapsed().as_millis()
    );
    unsafe { write_volatile(MARKER.add(4), 2) };
    cc_frontend::request(false);
    static EXECUTOR: static_cell::StaticCell<embassy_executor::Executor> =
        static_cell::StaticCell::new();
    unsafe {
        cortex_m::interrupt::enable();
    }
    EXECUTOR.init(embassy_executor::Executor::new()).run(|s| {
        measurement::init();
        s.spawn(usb_task(usb_driver, usb_route, s).unwrap());
        s.spawn(pd_tx::lease_task().unwrap());
        s.spawn(ui::task(aw).unwrap());
        s.spawn(cc_frontend::task(aw).unwrap());
        {
            s.spawn(measurement::task().unwrap());
            s.spawn(lines::task().unwrap());
        }
        rpc_pd::start();
    })
}

// TimerA1 is reserved by the HAL time driver; both sources use the same
// wrap-safe clock/queue handler. SysTick stays disabled after pre-init.
#[interrupt]
fn INT080() {
    embassy_hc32::time_driver::on_interrupt();
}
#[interrupt]
fn INT081() {
    embassy_hc32::time_driver::on_interrupt();
}

#[embassy_executor::task]
async fn usb_task(
    driver: embassy_usb_synopsys_otg::Driver<'static>,
    _route: embassy_hc32::intc::Route,
    _spawner: embassy_executor::Spawner,
) {
    {
        rpc::run(driver, _spawner).await;
    }
}

#[exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    return_to_loader(0xbad0_0001)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    struct RetentionWriter(usize);
    impl core::fmt::Write for RetentionWriter {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            for byte in text.bytes() {
                if self.0 >= 255 {
                    break;
                }
                unsafe {
                    core::ptr::write_volatile((0x200f0e00 as *mut u8).add(self.0), byte);
                }
                self.0 += 1;
            }
            Ok(())
        }
    }
    unsafe {
        for i in 0..256 {
            core::ptr::write_volatile((0x200f0e00 as *mut u8).add(i), 0);
        }
    }
    use core::fmt::Write;
    let _ = write!(RetentionWriter(0), "{info}");
    if let Some(loc) = info.location() {
        unsafe {
            write_volatile(MARKER.add(4), loc.line());
            for i in 0..24 {
                core::ptr::write_volatile((MARKER as *mut u8).add(20 + i), 0);
            }
            for (i, byte) in loc.file().bytes().enumerate().take(24) {
                core::ptr::write_volatile((MARKER as *mut u8).add(20 + i), byte);
            }
        }
    }
    return_to_loader(0xbad0_0002)
}
