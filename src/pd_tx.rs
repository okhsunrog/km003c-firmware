//! HW2.1 CC1 timer/DMA transmitter, shared by the fixed GetCaps bench probe and
//! the usbpd sink stack. The stack supplies Requests, GoodCRC and reset frames;
//! this module handles waveform completion, cancellation and CC termination.
//! PB6 follows native 2.0.7 mode 1.
use crate::pd_capture::{read8, read16, stop_events, write16};
use core::{
    cell::UnsafeCell,
    future::poll_fn,
    sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering, fence},
    task::Poll,
};
use km003c_drivers::pd::{
    Sop,
    tx::{self, Frame, Periods},
};

const TIMER: usize = 0x40015400;
const DMA: usize = 0x40053000;
const PA0: usize = 0x40053c00;
const PB6: usize = 0x40053c58;
const PC14: usize = 0x40053cb8;
const PC15: usize = 0x40053cbc;
// 0=idle, 1=DMA, 2=last interval, 3=wire done, 4=fault, 5=deadline.
static STATE: AtomicU8 = AtomicU8::new(0);
static LEASE: AtomicU32 = AtomicU32::new(0);
static LEASE_VERSION: AtomicU32 = AtomicU32::new(0);
static LEASE_CHANGED: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    Option<(embassy_time::Instant, u32)>,
> = embassy_sync::signal::Signal::new();

// Generation checks prevent expiry of an old lease from detaching a renewed
// connection. Changes and expiry are serialized with the hardware handoff.
fn arm_lease(ms: u64) {
    let version = LEASE_VERSION.load(Ordering::Relaxed).wrapping_add(1).max(1);
    LEASE_VERSION.store(version, Ordering::Relaxed);
    LEASE.store(version, Ordering::Release);
    LEASE_CHANGED.signal(Some((
        embassy_time::Instant::now() + embassy_time::Duration::from_millis(ms),
        version,
    )));
}
#[embassy_executor::task]
pub async fn lease_task() {
    use embassy_futures::select::{Either, select};
    let mut deadline = None;
    loop {
        deadline = if let Some((at, version)) = deadline {
            match select(LEASE_CHANGED.wait(), embassy_time::Timer::at(at)).await {
                Either::First(next) => next,
                Either::Second(()) => {
                    cortex_m::interrupt::free(|_| {
                        if LEASE.load(Ordering::Acquire) == version {
                            release();
                        }
                    });
                    None
                }
            }
        } else {
            LEASE_CHANGED.wait().await
        };
    }
}
// Cancelling a transmit future also stops active DMA/output. Completed TX
// keeps CC attached; only an unfinished waveform needs the release path.
struct ActiveTx;
impl Drop for ActiveTx {
    fn drop(&mut self) {
        if matches!(STATE.load(Ordering::Acquire), 1 | 2) {
            release();
        }
    }
}
static USED: AtomicBool = AtomicBool::new(false);
static START_CYCLES: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pd-sink")]
pub fn start_cycles() -> u32 {
    START_CYCLES.load(Ordering::Acquire)
}
static WAKER: embassy_sync::waitqueue::AtomicWaker = embassy_sync::waitqueue::AtomicWaker::new();
struct Storage(UnsafeCell<[u16; 860]>);
// Only the capture executor writes, while TX DMA is quiescent. Static storage
// remains valid even if a failed bus master cannot be proved stopped.
unsafe impl Sync for Storage {}
static BUFFER: Storage = Storage(UnsafeCell::new([0; 860]));
fn r(a: usize) -> u32 {
    unsafe { (a as *const u32).read_volatile() }
}
fn w(a: usize, v: u32) {
    unsafe { (a as *mut u32).write_volatile(v) }
}

pub fn attached() -> bool {
    LEASE.load(Ordering::Acquire) != 0
}
#[cfg(feature = "pd-sink")]
pub fn extend_lease() {
    cortex_m::interrupt::free(|_| {
        if attached() {
            arm_lease(30_000);
        }
    });
}
fn bounded_measurements() -> Result<(), &'static str> {
    let crate::measurement::State::Sample(sample) = crate::measurement::snapshot() else {
        return Err("INA229 unavailable");
    };
    let Some(current) = crate::measurement::uncalibrated_microamps(sample) else {
        return Err("INA229 invalid");
    };
    if sample.bus_nanovolts() > 5_500_000_000 || current.unsigned_abs() > 100_000 {
        return Err("requires unloaded VBUS below 5.5 V");
    }
    crate::lines::fresh().ok_or("line ADC stale")?;
    Ok(())
}

/// Must be called only by the capture owner after RX is armed on CC1.
pub fn attach() -> Result<(), &'static str> {
    bounded_measurements()?;
    let s = crate::lines::fresh().ok_or("line ADC stale")?;
    if s.uv[0] < 2_500_000 || s.uv[1] > 400_000 || attached() {
        return Err("requires detached charger on CC1 only");
    }
    cortex_m::interrupt::free(|_| {
        USED.store(false, Ordering::Release);
        arm_lease(5000);
        // PB6 push-pull, default drive, no pull-up: native 0x40b12(1).
        // Latch first, then enable. Do not touch AW9523 or the other CC line.
        write16(0x40053818, 1 << 6);
        write16(0x40053bfc, 0xa501);
        write16(PB6 + 2, 0);
        write16(PB6, 3);
        write16(0x40053bfc, 0xa500);
    });
    Ok(())
}

fn stop_tx_dma() -> bool {
    w(DMA + 0x1c, r(DMA + 0x1c) & !2);
    fence(Ordering::SeqCst);
    (0..2000).any(|_| r(DMA + 0x24) & (2 << 16) == 0)
}
fn outputs_off() {
    write16(TIMER + 0x140, 0xe0); // OUTEN off, no peripheral waveform at PA0.
    write16(0x40053bfc, 0xa501);
    write16(PC14, read16(PC14) & !2);
    write16(PA0 + 2, 0);
    write16(PA0, 0x8000); // Restore ADC/CMP analog input, digital output off.
    write16(0x40053bfc, 0xa500);
}
/// Safe from timeout, cancellation, fault/reset or CDC disconnect, even before RX init.
/// DMA is stopped before any later buffer reuse; no stack memory is involved.
pub fn release() {
    cortex_m::interrupt::free(|_| {
        let leased = LEASE.swap(0, Ordering::AcqRel) != 0;
        LEASE_CHANGED.signal(None);
        if matches!(STATE.load(Ordering::Acquire), 1 | 2) {
            stop_events();
            let _ = stop_tx_dma();
            outputs_off();
            STATE.store(5, Ordering::Release);
            WAKER.wake();
        }
        if leased {
            write16(0x4005381a, 1 << 6); // Native released state: PB6 low.
        }
    });
}
/// Timer/DMA IRQs own the DMA -> last-interval -> RX handoff.
pub fn on_interrupt(kind: u8) -> bool {
    // Serialize with the lease timeout cleanup as well as other DMA IRQs.
    // Otherwise expiry could interrupt the tail handoff and be overwritten by
    // its stale state transition. The body is bounded and never awaits.
    cortex_m::interrupt::free(|_| on_interrupt_locked(kind))
}
fn on_interrupt_locked(kind: u8) -> bool {
    let state = STATE.load(Ordering::Acquire);
    if !matches!(state, 1 | 2) {
        return false;
    }
    match kind {
        1 if state == 1 && r(DMA + 8) & 2 != 0 => {
            w(DMA + 0x1c, r(DMA + 0x1c) & !2);
            w(DMA + 0x18, 2 | (2 << 16));
            // The final reload is already loaded. Hold its output, clear the
            // old overflow flag and wait for that LAST interval to elapse.
            write16(TIMER + 0x140, (read16(TIMER + 0x140) & !0xc0) | 0x80);
            STATE.store(2, Ordering::Release);
            crate::pd_capture::write8(TIMER + 0x81, 0x10);
        }
        0 if state == 2 && read8(TIMER + 0x81) & 0x40 != 0 => {
            stop_events();
            let quiet = stop_tx_dma();
            outputs_off();
            STATE.store(if quiet { 3 } else { 4 }, Ordering::Release);
            if quiet {
                // No executor/CDC scheduling gap before the source's GoodCRC.
                crate::pd_buffered::resume_after_tx();
            } else {
                release();
            }
            WAKER.wake();
        }
        2 if r(DMA + 4) & (2 | (2 << 16)) != 0 => {
            release();
            w(DMA + 0x14, 2 | (2 << 16));
            STATE.store(4, Ordering::Release);
            WAKER.wake();
        }
        _ => (),
    }
    true
}

/// One fixed PD3.0 sink/UFP MessageID=0 control frame per five-second lease.
/// This asks for capabilities; it cannot select a voltage or enter EPR.
pub async fn get_caps(rx: &mut crate::pd_buffered::Buffered) -> Result<usize, &'static str> {
    bounded_measurements()?;
    let s = crate::lines::fresh().ok_or("line ADC stale")?;
    if !attached() || !(400_000..=2_150_000).contains(&s.uv[0]) || s.uv[1] > 400_000 {
        return Err("CC1 sink lease required");
    }
    // PC14/15 are oscillator pads. Never override an active XTAL32 source.
    if read8(0x40054420) & 1 == 0 {
        return Err("XTAL32 still active");
    }
    if USED.swap(true, Ordering::AcqRel) {
        return Err("one probe per lease");
    }
    transmit(
        rx,
        Frame::Packet {
            sop: Sop::Sop,
            wire: &[0x87, 0x00],
        },
    )
    .await
}

/// Shared hardware engine; the sink stack owns message framing/IDs/GoodCRC.
pub async fn transmit(
    rx: &mut crate::pd_buffered::Buffered,
    frame: Frame<'_>,
) -> Result<usize, &'static str> {
    if !attached() {
        return Err("CC lease expired");
    }
    if read8(0x40054420) & 1 == 0 {
        return Err("XTAL32 still active");
    }
    if !stop_tx_dma() {
        return Err("TX DMA not stopped");
    }
    let count = tx::encode(frame, Periods::NATIVE_207, unsafe { &mut *BUFFER.0.get() })
        .map_err(|_| "encode failed")?;
    // A running capture timer means recent activity. Do not cut into a frame.
    let due = embassy_time::Instant::now() + embassy_time::Duration::from_millis(50);
    loop {
        let idle = cortex_m::interrupt::free(|_| {
            if read16(TIMER + 0x80) & 1 != 0 || read16(0x4004a004) & 1 == 0 {
                return false;
            }
            rx.stop()
        });
        if idle {
            break;
        }
        if embassy_time::Instant::now() >= due {
            return Err("CC busy");
        }
        embassy_time::Timer::after_millis(1).await;
    }
    let _active = ActiveTx;
    let result = cortex_m::interrupt::free(|_| {
        if !attached() {
            return Err("lease expired");
        }
        // Native PA0 AF4 configuration has POUTE=0; Timer OUTEN owns waveform.
        write16(0x40053bfc, 0xa501);
        write16(PC14, 0);
        write16(PC15, 0);
        write16(PC14 + 2, 0);
        write16(PA0, 0);
        write16(PA0 + 2, 4);
        write16(0x40053bfc, 0xa500);
        write16(0x4005382a, 1 << 14);
        write16(0x40053826, read16(0x40053826) | (1 << 14));
        write16(TIMER + 0x100, 0); // Channel 1 PWM, not capture.
        write16(TIMER + 0x140, 0x10e0);
        w(DMA + 0x14, 2 | (2 << 16));
        w(DMA + 0x18, 2 | (2 << 16));
        let ptr = BUFFER.0.get().cast::<u16>();
        w(DMA + 0x80, unsafe { ptr.add(1) } as u32);
        w(DMA + 0x84, (TIMER + 4) as u32);
        w(DMA + 0x88, (((count - 1) as u32) << 16) | 1);
        w(DMA + 0x9c, 0x1101); // halfword, source++, fixed PERAR, IRQ.
        w(0x40010818, 0x103);
        w(DMA + 0xc, r(DMA + 0xc) & !(2 | (2 << 16)));
        w(DMA + 0x10, (r(DMA + 0x10) & !2) | (2 << 16));
        write16(TIMER + 4, unsafe { ptr.read() });
        write16(TIMER, 0);
        STATE.store(1, Ordering::Release);
        fence(Ordering::SeqCst);
        w(DMA + 0x1c, r(DMA + 0x1c) | 2);
        START_CYCLES.store(r(0xe0001004), Ordering::Release);
        write16(TIMER + 0x80, (read16(TIMER + 0x80) & !0x5001) | 1);
        Ok(())
    });
    result?;
    if embassy_time::with_timeout(
        embassy_time::Duration::from_millis(5),
        poll_fn(|cx| {
            WAKER.register(cx.waker());
            if matches!(STATE.load(Ordering::Acquire), 1 | 2) {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }),
    )
    .await
    .is_err()
    {
        release();
    }
    if STATE.load(Ordering::Acquire) == 3 {
        Ok(count)
    } else {
        Err("TX fault or deadline")
    }
}
