use embassy_hc32::{intc, pac};
use embassy_usb_synopsys_otg::{Config, Driver, OtgInstance, PhyType, StateStorage, otg_v1::Otg};
use pac::interrupt;
static STATE: StateStorage<6> =
    StateStorage::new(embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex::new());

#[pac::interrupt]
fn INT000() {
    unsafe {
        let p = 0x200f0fd8 as *mut u32;
        p.write_volatile(p.read_volatile().wrapping_add(1));
    }
    unsafe {
        embassy_usb_synopsys_otg::on_interrupt(
            Otg::from_ptr(0x400c0000 as *mut _),
            &STATE.as_state(),
        );
    }
}

// Configure chip clocks, pin mux and FIFO allocation before starting the
// Synopsys driver. The native loader can leave peripheral state behind.
pub unsafe fn driver(buffer: &'static mut [u8]) -> (Driver<'static>, intc::Route) {
    use core::ptr::{read_volatile as r, write_volatile as w};
    unsafe {
        let pll =
            embassy_hc32::clock_decode::pll_from_pllcfgr(r(0x40054100 as *const u32)).unwrap();
        let input = 16_000_000u64;
        assert_eq!(
            input * u64::from(pll.n)
                / u64::from(pll.m)
                / u64::from(((r(0x40054100 as *const u32) >> 20) & 15) + 1),
            48_000_000
        );
        // Select the verified 48 MHz PLLR output.
        w(0x400543fe as *mut u16, 0xa501);
        w(0x40054024 as *mut u8, 0xa0);
        w(0x400543fe as *mut u16, 0xa500);
        w(0x40053bfc as *mut u16, 0xa501);
        for addr in [0x40053c26, 0x40053c2e, 0x40053c32] {
            w(addr as *mut u16, 10);
        }
        w(0x40053bfc as *mut u16, 0xa500);
        w(0x40048004 as *mut u32, r(0x40048004 as *const u32) & !0x100);
        assert_eq!(r(0x400c0040 as *const u32), 0x4f54330a);
        // Disconnect inherited USB before resetting its FIFOs and endpoint state.
        w(0x400c0008 as *mut u32, 0);
        w(0x400c0018 as *mut u32, 0);
        w(0x400c0804 as *mut u32, r(0x400c0804 as *const u32) | 2);
        cortex_m::asm::delay(2_000_000);
        w(
            0x400c000c as *mut u32,
            r(0x400c000c as *const u32) | (1 << 6),
        );
        cortex_m::asm::delay(1_000_000);
        wait(0x400c0010, 1 << 31, true);
        w(0x400c0010 as *mut u32, 1);
        wait(0x400c0010, 1, false);
        cortex_m::asm::delay(1000);
        // Internal FS PHY and forced device mode; turnaround at HCLK >= 32MHz.
        w(0x400c000c as *mut u32, (1 << 30) | (1 << 6) | (9 << 10));
        wait(0x400c0014, 1, false);
        // Match the native loader's documented-by-disassembly B-session setting.
        w(
            0x400c0038 as *mut u32,
            r(0x400c0038 as *const u32) | (1 << 19),
        );
        w(0x400c0e00 as *mut u32, 0);
        let route = intc::Route::new(0x113, intc::Line::new(0).unwrap(), 8).unwrap();
        let instance = OtgInstance {
            regs: Otg::from_ptr(0x400c0000 as *mut _),
            state: STATE.as_state(),
            fifo_depth_words: 308,
            tx_fifo_count: 6,
            phy_type: PhyType::InternalFullSpeed,
            // Account for RX status/setup overhead in addition to OUT payloads.
            // EP0 + bulk OUT contribute 32 words; this yields the native 64-word
            // RX allocation. A zero reserve reproducibly stalled enumeration.
            extra_rx_fifo_words: 32,
            calculate_trdt_fn: |_| 9,
        };
        (Driver::new(buffer, instance, Config::default()), route)
    }
}

unsafe fn wait(address: usize, mask: u32, set: bool) {
    for _ in 0..2_000_000 {
        if (unsafe { core::ptr::read_volatile(address as *const u32) } & mask != 0) == set {
            return;
        }
    }
    panic!("USB core timeout");
}
