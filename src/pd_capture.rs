//! Opt-in, single-burst passive PD capture. CDC commands start acquisition.
//!
//! Register settings follow native 1.9.9 init (device address 0x102c0), checked
//! against DDL 3.3.0 and an offline Unicorn write trace. HW2.1 captured a
//! CRC-valid PD on PA0/CC1 and PA1/CC2. This receiver never drives CC itself;
//! the separate opt-in pd-tx-test owner temporarily borrows its stopped timer.
use core::{
    future::poll_fn,
    sync::atomic::{AtomicU32, Ordering},
    task::Poll,
};
use embassy_futures::select::{Either, select};
use embassy_hc32::{
    dma::{self, Channel, Unit},
    intc::{Line, Route},
    pac::interrupt,
};
use embassy_sync::waitqueue::AtomicWaker;
use embassy_time::{Duration, with_timeout};

const TMRA: usize = 0x40015400;
const CMP: usize = 0x4004a000;
const DCU: usize = 0x40052000;
const WAITING: u32 = u32::MAX;
static END: AtomicU32 = AtomicU32::new(WAITING);
static WAKER: AtomicWaker = AtomicWaker::new();
pub(crate) fn read8(address: usize) -> u8 {
    unsafe { (address as *const u8).read_volatile() }
}
pub(crate) fn write8(address: usize, value: u8) {
    unsafe { (address as *mut u8).write_volatile(value) }
}
pub(crate) fn read16(address: usize) -> u16 {
    unsafe { (address as *const u16).read_volatile() }
}
pub(crate) fn write16(address: usize, value: u16) {
    unsafe { (address as *mut u16).write_volatile(value) }
}
fn read32(address: usize) -> u32 {
    unsafe { (address as *const u32).read_volatile() }
}
fn write32(address: usize, value: u32) {
    unsafe { (address as *mut u32).write_volatile(value) }
}

/// CMP1_INP1/2 reach CC1/CC2 on the hardware-tested HW2.1 board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Cmp1Inp1,
    Cmp1Inp2,
}
impl Input {
    pub fn number(self) -> u8 {
        match self {
            Self::Cmp1Inp1 => 1,
            Self::Cmp1Inp2 => 2,
        }
    }
}
#[derive(Debug)]
pub enum Error {
    Dma(dma::Error),
    Routing(embassy_hc32::intc::IrqError),
    InvalidBuffer,
    Timeout,
    BufferFull,
    CountMismatch { events: usize, transferred: usize },
}
pub(crate) fn stop_events() {
    // Disable edge events and the physical VCOUT port, retain internal comparator
    // output only. Then stop the timer and disable/clear overflow IRQ state.
    write16(CMP, read16(CMP) & !0x1060);
    write16(TMRA + 0x80, read16(TMRA + 0x80) & !1);
    write8(TMRA + 0x81, read8(TMRA + 0x81) & !0x50);
}
struct StopEvents;
impl Drop for StopEvents {
    fn drop(&mut self) {
        stop_events();
    }
}

#[interrupt]
fn INT008() {
    #[cfg(feature = "pd-tx-test")]
    if crate::pd_tx::on_interrupt(0) {
        return;
    }
    #[cfg(feature = "cdc-telemetry")]
    if crate::pd_buffered::enabled() {
        crate::pd_buffered::on_interrupt(0);
        return;
    }
    if read8(TMRA + 0x81) & 0x40 != 0 {
        stop_events();
        END.store(read32(DCU + 8) & 0xffff, Ordering::Release);
        WAKER.wake();
    }
}
#[cfg(feature = "pd-tx-test")]
#[interrupt]
fn INT016() {
    crate::pd_tx::on_interrupt(1);
}
#[interrupt]
fn INT017() {
    #[cfg(feature = "cdc-telemetry")]
    if crate::pd_buffered::enabled() {
        crate::pd_buffered::on_interrupt(1);
        return;
    }
    dma::on_interrupt(Unit::Dma1);
}
#[interrupt]
fn INT018() {
    #[cfg(feature = "pd-tx-test")]
    if crate::pd_tx::on_interrupt(2) {
        return;
    }
    #[cfg(feature = "cdc-telemetry")]
    if crate::pd_buffered::enabled() {
        crate::pd_buffered::on_interrupt(2);
        return;
    }
    dma::on_interrupt(Unit::Dma1);
}

pub struct Receiver {
    channel: Channel,
    _routes: [Route; if cfg!(feature = "pd-tx-test") { 4 } else { 3 }],
}
impl Receiver {
    /// Passive snapshot after capture cancellation; no register reads clear
    /// flags here. Field order is documented in the CDC protocol notes.
    pub fn diagnostics(&self) -> [u32; 12] {
        [
            u32::from(read16(CMP)),
            u32::from(read16(CMP + 2)),
            u32::from(read16(CMP + 4)),
            read32(TMRA + 0x80),
            u32::from(read16(TMRA)),
            u32::from(read16(TMRA + 0x48)),
            read32(DCU + 8),
            read32(0x400530e8),
            read32(0x4001084c),
            read32(0x40010850),
            read32(0x40053010),
            read32(0x40053014),
        ]
    }

    /// Switch only the internal comparator mux between captures. Event/output
    /// drivers stay disabled; this does not change termination or any GPIO.
    pub fn select_input(&mut self, input: Input) {
        stop_events();
        let positive = match input {
            Input::Cmp1Inp1 => 0x100,
            Input::Cmp1Inp2 => 0x200,
        };
        write16(CMP + 2, 0x4004 | positive);
    }

    /// Feed a captured burst into the independent decoder. Errors are emitted
    /// alongside valid packets so callers can measure loss/noise rather than
    /// silently accepting a damaged trace. A quiet gap always resets state.
    pub async fn receive(
        &mut self,
        buffer: &mut [u16],
        decoder: &mut km003c_drivers::pd::Decoder,
        timeout: Duration,
        mut event: impl FnMut(Result<km003c_drivers::pd::Event, km003c_drivers::pd::Error>),
    ) -> Result<usize, Error> {
        decoder.reset();
        let count = self.capture(buffer, timeout).await?;
        for &ticks in &buffer[..count] {
            match decoder.push_interval(ticks) {
                Ok(Some(packet)) => event(Ok(packet)),
                Ok(None) => (),
                Err(error) => event(Err(error)),
            }
        }
        if let Err(error) = decoder.finish() {
            event(Err(error));
        }
        Ok(count)
    }

    /// # Safety
    /// Own TMRA2, CMP1/DAC1, DCU1, **all DMA1 channels**, corresponding AOS
    /// trigger selectors and IRQ8/17/18 (also IRQ16 with pd-tx-test). Configure the independently verified
    /// analog input pad before calling. No task/IRQ may alter those resources.
    /// Keep all CC transmit/termination GPIO outputs disabled separately, except
    /// when the exclusive board TX owner pauses RX and performs its handoff.
    pub unsafe fn new(input: Input, dac_code: u8) -> Result<Self, Error> {
        let [_, _, channel, _] = unsafe { dma::split(Unit::Dma1) }.map_err(Error::Dma)?;
        // First stop inherited event producers before routing CPU interrupts.
        write32(0x40048008, read32(0x40048008) & !8); // FCG2.TMRA2
        write32(0x4004800c, read32(0x4004800c) & !0x110); // FCG3.CMP/DAC
        stop_events();
        write16(CMP, 0); // Comparator off while configuring its reference/input.
        write32(0x40048010, 0xa5a50001);
        write32(0x40048000, read32(0x40048000) & !(1 << 24)); // DCU1
        write32(0x40048010, 0xa5a50000);
        let routes = unsafe {
            [
                Route::new(0x103, Line::new(8).unwrap(), 5).map_err(Error::Routing)?,
                #[cfg(feature = "pd-tx-test")]
                Route::new(Unit::Dma1.complete_event(1).0, Line::new(16).unwrap(), 5)
                    .map_err(Error::Routing)?,
                Route::new(Unit::Dma1.complete_event(2).0, Line::new(17).unwrap(), 5)
                    .map_err(Error::Routing)?,
                Route::new(Unit::Dma1.error_event().0, Line::new(18).unwrap(), 5)
                    .map_err(Error::Routing)?,
            ]
        };
        // Timer counts up, PCLK1/8. Native event starts AND clears the timer,
        // and channel 3 captures each CMP1 event, so samples are edge intervals.
        write16(TMRA + 0x80, 0x32);
        write8(TMRA + 0x81, 0);
        write16(TMRA + 0x84, 0x404); // HSTA2 + HCLE2, common event
        write16(TMRA + 0x88, 0); // Disable alternate count sources.
        write16(TMRA + 0x8c, 0);
        write16(TMRA + 0x94, 4); // Capture IRQ source enable used by native path.
        write16(TMRA + 0x108, 0x4041); // CCONR3: capture mode, common event
        // Never enable the timer's external waveform outputs (PCONR1..8).
        for channel in 0..8 {
            write16(TMRA + 0x140 + 4 * channel, 0);
        }
        write32(0x4001084c, 0x1a0);
        write32(0x40010850, 0x1a0);
        write32(DCU, 0x0b); // 16-bit hardware addition, DATA0 += DATA1
        write32(DCU + 12, 1);
        write32(0x40010804, 0x105); // Capture event -> DCU1
        // DAC1 is shared by comparators: caller explicitly chooses its code.
        unsafe {
            (0x4004a100 as *mut u8).write_volatile(dac_code);
        }
        write16(0x4004a108, read16(0x4004a108) | 1);
        let positive = match input {
            Input::Cmp1Inp1 => 0x100,
            Input::Cmp1Inp2 => 0x200,
        };
        write16(CMP + 2, 0x4004 | positive); // Native DAC negative-input selection
        write16(CMP + 6, 5);
        write16(CMP + 8, 0x0f);
        // Native 0xd065, with OUTEN removed and edges disabled until capture.
        // CMPOE is internal comparison output; OUTEN would drive VCOUT externally.
        write16(CMP, 0xc005);
        Ok(Self {
            channel,
            _routes: routes,
        })
    }

    /// Capture one burst, ending on timer overflow after a quiet interval.
    /// There is a blind interval before the caller rearms; this is not a
    /// lossless analyzer. Buffer-full and DMA/event count mismatch are errors.
    /// Cancellation drops the HAL transfer, waiting for DMA bus quiescence
    /// before releasing the borrowed destination. A wedged DMA may stall there.
    pub async fn capture(&mut self, buffer: &mut [u16], timeout: Duration) -> Result<usize, Error> {
        if buffer.is_empty() || buffer.len() > 860 {
            return Err(Error::InvalidBuffer);
        }
        stop_events();
        let capacity = buffer.len();
        END.store(WAITING, Ordering::Release);
        write32(DCU + 8, 0);
        write16(TMRA, 0);
        write16(TMRA + 4, 0x98); // Quiet-gap period from native receive setup
        // The HAL owns DMA cancellation and memory fences. No custom raw DMA
        // pointer is retained after the borrowed transfer is dropped.
        let transfer = unsafe {
            self.channel.read_u16(
                (TMRA + 0x48) as *const u16,
                buffer,
                embassy_hc32::pac::events::EventSource(0x105),
            )
        }
        .map_err(Error::Dma)?;
        let guard = StopEvents;
        // BCSTRH is the byte at +0x81, not a halfword at +0x82.
        write8(TMRA + 0x81, 0x10); // Overflow IRQ, stale flags cleared
        write16(CMP, read16(CMP) | 0x60); // Both edges, no CPU comparator IRQ
        let end = poll_fn(|cx| {
            WAKER.register(cx.waker());
            match END.load(Ordering::Acquire) {
                WAITING => Poll::Pending,
                n => Poll::Ready(n as usize),
            }
        });
        let result = select(transfer, with_timeout(timeout, end)).await;
        drop(guard);
        match result {
            Either::First(Err(e)) => Err(Error::Dma(e)),
            Either::First(Ok(())) => Err(Error::BufferFull),
            Either::Second(Err(_)) => Err(Error::Timeout),
            Either::Second(Ok(events)) => {
                // A DCU event alone does not prove a DMA write succeeded. Check
                // the settled transfer count before exposing buffer contents.
                // DTCTL2 holds the programmed reload count; MONDTCTL2 is
                // the live remaining count (DDL DMA_GetTransCount).
                let remaining = (read32(0x400530e8) >> 16) as usize;
                let transferred = capacity.saturating_sub(remaining);
                if events != transferred || events > capacity {
                    Err(Error::CountMismatch {
                        events,
                        transferred,
                    })
                } else {
                    Ok(events)
                }
            }
        }
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        stop_events();
        write16(CMP, 0);
        // Preserve DAC1 state: caller owns it, other comparator users may share
        // its reference after explicitly transferring ownership elsewhere.
    }
}
