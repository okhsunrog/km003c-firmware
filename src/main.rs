#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use cortex_m_rt::{entry, exception};

mod aw9523;
mod display;
#[cfg(feature = "measurement")]
pub mod measurement;
#[cfg(feature = "pd-capture")]
pub mod pd_capture;
mod retention_log;
#[cfg(feature = "cdc-telemetry")]
mod telemetry;
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
    let fields: [&[u8]; 3] = [
        b"KM003C",
        if cfg!(feature = "measurement") {
            b"rust-adc"
        } else {
            b"rust-ui"
        },
        b"2026.10.28",
    ];
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
    cortex_m::interrupt::disable();
    unsafe {
        write_volatile(MARKER.add(3), status);
        write_volatile(BOOT_REQUEST, 0x1234_5678);
    }
    cortex_m::asm::dsb();
    cortex_m::peripheral::SCB::sys_reset()
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
    let cp = cortex_m::Peripherals::take().unwrap();
    use embassy_hc32::clocks::{self, Config, Divider, Dividers, Mpll};
    unsafe {
        write_volatile(0x40050804 as *mut u32, 0x77);
        write_volatile(0x40050800 as *mut u32, 0x22222222);
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
    embassy_hc32::time_driver::init(cp.SYST, hclk).unwrap();
    static EXECUTOR: static_cell::StaticCell<embassy_executor::Executor> =
        static_cell::StaticCell::new();
    unsafe {
        cortex_m::interrupt::enable();
    }
    EXECUTOR.init(embassy_executor::Executor::new()).run(|s| {
        #[cfg(feature = "measurement")]
        measurement::init();
        s.spawn(usb_task().unwrap());
        s.spawn(ui::task().unwrap());
        #[cfg(feature = "measurement")]
        s.spawn(measurement::task().unwrap());
        #[cfg(feature = "cdc-telemetry")]
        s.spawn(telemetry::measurements().unwrap());
        #[cfg(all(feature = "cdc-telemetry", feature = "pd-capture"))]
        s.spawn(telemetry::captures().unwrap());
    })
}

#[exception]
fn SysTick() {
    embassy_hc32::time_driver::systick_tick();
    static TICKS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
    let deadline = if cfg!(feature = "manual-ui-test") {
        600_000
    } else {
        90_000
    };
    if TICKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed) >= deadline {
        unsafe {
            for (i, address) in [
                0x400c0008usize,
                0x400c000c,
                0x400c0014,
                0x400c0018,
                0x400c0038,
                0x400c0800,
                0x400c0804,
                0x400c0808,
                0x40048004,
                0x400c0040,
                0x40054024,
                0x40054100,
                0x400c0b00,
                0x400c0b08,
                0x400c0b10,
                0x400c0900,
                0x400c0908,
                0x400c0910,
                0x400c0810,
                0x400c0814,
                0x400c0818,
                0x400c081c,
                0x400c0024,
                0x400c0028,
            ]
            .into_iter()
            .enumerate()
            {
                write_volatile(
                    (0x200f0e00 as *mut u32).add(i),
                    core::ptr::read_volatile(address as *const u32),
                );
            }
        }
        return_to_loader(0x600d0002);
    }
}

#[embassy_executor::task]
async fn usb_task() {
    unsafe {
        write_volatile(MARKER.add(4), 1);
    }
    static OUT: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    let (driver, _route) = unsafe { usb::driver(OUT.init([0; 256])) };
    unsafe {
        write_volatile(MARKER.add(4), 2);
    }
    let mut config = embassy_usb::Config::new(0x5fc9, 0x0063);
    config.manufacturer = Some("KM003C experimental");
    config.product = Some(if cfg!(feature = "cdc-telemetry") {
        "KM003C Rust telemetry"
    } else {
        "KM003C Rust USB test"
    });
    config.serial_number = Some("rust-bringup");
    let mut cd = [0; 256];
    let mut bd = [0; 256];
    let mut control = [0; 64];
    let mut state = embassy_usb::class::cdc_acm::State::new();
    let mut msos = [0; 0];
    let mut builder =
        embassy_usb::Builder::new(driver, config, &mut cd, &mut bd, &mut msos, &mut control);
    let serial = embassy_usb::class::cdc_acm::CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut device = builder.build();
    unsafe {
        write_volatile(MARKER.add(4), 3);
    }
    #[cfg(feature = "cdc-telemetry")]
    {
        let (sender, receiver) = serial.split();
        embassy_futures::join::join3(
            device.run(),
            telemetry::receive(receiver),
            telemetry::send(sender),
        )
        .await;
    }
    #[cfg(not(feature = "cdc-telemetry"))]
    {
        let (mut sender, mut receiver) = serial.split();
        let queue = embassy_sync::channel::Channel::<
            embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
            ([u8; 64], usize),
            8,
        >::new();
        let receive = async {
            loop {
                receiver.wait_connection().await;
                let mut data = [0; 64];
                while let Ok(n) = receiver.read_packet(&mut data).await {
                    queue.send((data, n)).await;
                }
            }
        };
        let send = async {
            loop {
                let (data, n) = queue.receive().await;
                sender.wait_connection().await;
                if sender.write_packet(&data[..n]).await.is_err() {
                    log::warn!("CDC send interrupted by disconnect");
                }
            }
        };
        embassy_futures::join::join3(device.run(), receive, send).await;
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
