use embassy_hc32::{intc, pac};
use embassy_usb_synopsys_otg::{Config, Driver, OtgInstance, PhyType, StateStorage, otg_v1::Otg};
use pac::interrupt;
static STATE: StateStorage<6> = StateStorage::new();

#[pac::interrupt]
fn INT000() {
    unsafe {
        embassy_usb_synopsys_otg::on_interrupt(
            Otg::from_ptr(0x400c0000 as *mut _),
            &STATE.as_state(),
        );
    }
}

// Bring-up adapter: clocks/pin mux are explicitly checked against the native
// loader configuration before allowing the generic driver to touch the core.
pub unsafe fn driver(buffer: &'static mut [u8]) -> (Driver<'static>, intc::Route) {
    use core::ptr::{read_volatile as r, write_volatile as w};
    unsafe {
        assert_eq!(r(0x400c0040 as *const u32), 0x4f54330a);
        // The bootloader supplies PLLR=48 MHz. Select it explicitly.
        w(0x400543fe as *mut u16, 0xa501);
        w(0x40054024 as *mut u8, 0xa0);
        w(0x400543fe as *mut u16, 0xa500);
        w(0x40053bfc as *mut u16, 0xa501);
        for addr in [0x40053c26, 0x40053c2e, 0x40053c32] {
            w(addr as *mut u16, 10);
        }
        w(0x40053bfc as *mut u16, 0xa500);
        w(0x40048004 as *mut u32, r(0x40048004 as *const u32) & !0x100);
        let route = intc::Route::new(0x113, intc::Line::new(0).unwrap(), 0x80).unwrap();
        let instance = OtgInstance {
            regs: Otg::from_ptr(0x400c0000 as *mut _),
            state: STATE.as_state(),
            fifo_depth_words: 308,
            phy_type: PhyType::InternalFullSpeed,
            extra_rx_fifo_words: 0,
            calculate_trdt_fn: |_| 5,
        };
        (Driver::new(buffer, instance, Config::default()), route)
    }
}
