//! Board-owned passive ADC1 scan, independent of INA229 and PD edge capture.
//! Never enables CC termination, PA0/1 transmit or PA2/3 protocol outputs.
use core::{
    cell::Cell,
    sync::atomic::{AtomicU8, AtomicU32, Ordering},
};
use embassy_hc32::{
    adc::InternalReference,
    gpio::{AnalogPin, Port},
    pac,
};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
pub use km003c_drivers::lines::Sample;

static LATEST: Mutex<CriticalSectionRawMutex, Cell<Option<Sample>>> = Mutex::new(Cell::new(None));
static INPUT: AtomicU8 = AtomicU8::new(0);
static DETACHED_INPUT: AtomicU8 = AtomicU8::new(0);
static UPDATED: AtomicU32 = AtomicU32::new(0);
pub fn fresh() -> Option<Sample> {
    let age = (embassy_time::Instant::now().as_millis() as u32)
        .wrapping_sub(UPDATED.load(Ordering::Acquire));
    (age < 100).then(snapshot).flatten()
}
pub fn snapshot() -> Option<Sample> {
    LATEST.lock(Cell::get)
}
pub fn cc_input() -> Option<u8> {
    match INPUT.load(Ordering::Relaxed) {
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

pub fn detached_input() -> Option<u8> {
    match DETACHED_INPUT.load(Ordering::Acquire) {
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

fn stop(adc: &pac::Adc1) -> bool {
    for _ in 0..10_000 {
        if adc.str().read().strt().bit_is_clear() {
            return true;
        }
        adc.str().write(|w| w.strt().clear_bit());
    }
    false
}

#[embassy_executor::task]
pub async fn task() {
    // This task owns ADC1 and initializes the pads. The PD transmitter may
    // temporarily switch PA0/PA1 to AF4, restoring analog mode before RX resumes.
    // No later ADC operation writes a pad configuration. CMP1 observes in parallel;
    // its DAC reference does not change CMP.RVADC or the ADC reference buffer.
    let adc = unsafe { pac::Adc1::steal() };
    let _pins = unsafe { [0, 1, 2, 3].map(|n| AnalogPin::new(Port::A, n)) };
    let pwc = unsafe { pac::Pwc::steal() };
    cortex_m::interrupt::free(|_| pwc.fcg3().modify(|_, w| w.adc1().clear_bit()));
    if !stop(&adc) {
        log::error!("Line ADC did not stop");
        return;
    }
    let _reference = unsafe { InternalReference::new().await };
    adc.icr().write(|w| unsafe { w.bits(0) });
    adc.isr().write(|w| unsafe { w.bits(0) });
    // DDL: 12-bit right-aligned, sequence A once; 16 hardware averages.
    adc.cr0().write(|w| unsafe { w.bits(0x300) });
    adc.cr1().write(|w| w.rschsel().set_bit());
    adc.trgsr().write(|w| unsafe { w.bits(0) });
    adc.synccr().write(|w| unsafe { w.bits(0x0c00) });
    adc.awdcr().write(|w| unsafe { w.bits(0) });
    adc.pgacr().write(|w| unsafe { w.bits(0) });
    adc.pgainsr1().write(|w| unsafe { w.bits(0) });
    adc.chmuxr0().write(|w| unsafe { w.bits(0x3210) });
    adc.chselra().write(|w| unsafe { w.bits(0x1000f) });
    adc.chselrb().write(|w| unsafe { w.bits(0) });
    adc.avchselr().write(|w| unsafe { w.bits(0x1000f) });
    // Stock uses 160 cycles on the four pads. Give the reference the same
    // settling window rather than assuming a low impedance signal source.
    unsafe {
        for n in 0..4 {
            adc.sstr0().as_ptr().add(n).write_volatile(160);
        }
    }
    adc.sstrl().write(|w| unsafe { w.bits(160) });
    let (mut candidate, mut repeats) = ((None, None), 0u8);
    loop {
        adc.isr().modify(|_, w| w.eocaf().clear_bit());
        adc.str().write(|w| w.strt().set_bit());
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_millis(5);
        // A scan runs in hardware, while the CPU sleeps or services USB/PD.
        while adc.isr().read().eocaf().bit_is_clear() && embassy_time::Instant::now() < deadline {
            embassy_time::Timer::after_millis(1).await;
        }
        let completed = adc.isr().read().eocaf().bit_is_set();
        if !stop(&adc) {
            LATEST.lock(|c| c.set(None));
            INPUT.store(0, Ordering::Relaxed);
            DETACHED_INPUT.store(0, Ordering::Release);
            log::error!("Line ADC failed to quiesce");
            return;
        }
        let sample = if completed {
            Sample::from_raw([
                adc.dr0().read().bits(),
                adc.dr1().read().bits(),
                adc.dr2().read().bits(),
                adc.dr3().read().bits(),
                adc.dr16().read().bits(),
            ])
        } else {
            None
        };
        LATEST.lock(|c| c.set(sample));
        UPDATED.store(
            embassy_time::Instant::now().as_millis() as u32,
            Ordering::Release,
        );
        let next = (
            sample.and_then(|s| s.cc_input()),
            sample.and_then(|s| s.detached_cc_input()),
        );
        if next == candidate {
            repeats = repeats.saturating_add(1);
        } else {
            candidate = next;
            repeats = 1;
        }
        // Two separate scans reject single-scan spikes during cable insertion.
        if repeats >= 2 {
            INPUT.store(candidate.0.unwrap_or(0), Ordering::Relaxed);
            DETACHED_INPUT.store(candidate.1.unwrap_or(0), Ordering::Release);
        }
        embassy_time::Timer::after_millis(20).await;
    }
}
