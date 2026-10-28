//! Board-owned, interrupt-rearmed two-buffer DMA. Buffers live for the whole
//! program, unlike the HAL's borrowed transfers. No raw DMA pointer can outlive
//! storage, and a Frame guard prevents reusing its slot while Rust reads it.
use crate::pd_capture::{self, Input, Receiver};
use core::{
    cell::{RefCell, UnsafeCell},
    future::poll_fn,
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering, fence},
    task::Poll,
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    waitqueue::AtomicWaker,
};
use km003c_drivers::pd_pool::Pool;
const N: usize = 860;
struct Buffers(UnsafeCell<[[u16; N]; 2]>);
// All access follows Pool ownership under STATE's critical section. DMA writes
// FILLING only; immutable slices are created only for an acquired READING slot.
unsafe impl Sync for Buffers {}
static BUFFERS: Buffers = Buffers(UnsafeCell::new([[0; N]; 2]));
static ENABLED: AtomicBool = AtomicBool::new(false);
static WAKER: AtomicWaker = AtomicWaker::new();
#[derive(Clone, Copy, Default)]
pub struct Meta {
    pub input: u8,
    pub count: usize,
    pub error: u8,
    pub sequence: u32,
    pub end_us: u64,
    pub end_cycles: u32,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub input_switches: u32,
    pub switch_discards: u32,
    pub frames: u32,
    pub overruns: u32,
    pub held_overruns: u32,
    pub noise: u32,
    pub full: u32,
    pub dma_errors: u32,
    pub mismatches: u32,
    pub stop_faults: u32,
    pub max_handler_cycles: u32,
}
#[derive(Default)]
struct State {
    input: u8,
    pool: Pool,
    meta: [Meta; 2],
    stats: Stats,
    running: bool,
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
fn r(a: usize) -> u32 {
    unsafe { (a as *const u32).read_volatile() }
}
fn w(a: usize, v: u32) {
    unsafe { (a as *mut u32).write_volatile(v) }
}
fn stop_dma() -> bool {
    w(0x4005301c, r(0x4005301c) & !4);
    fence(Ordering::SeqCst);
    // Static memory remains valid even on failure. Fail closed rather than expose
    // a buffer while the bus master might still be writing it.
    for _ in 0..2000 {
        if r(0x40053024) & (4 << 16) == 0 {
            return true;
        }
    }
    false
}
fn arm(slot: usize) {
    w(0x40053014, 4 | (4 << 16));
    w(0x40053018, 4 | (4 << 16));
    w(0x400530c0, 0x40015448);
    let ptr = unsafe { core::ptr::addr_of_mut!((*BUFFERS.0.get())[slot]).cast::<u16>() };
    w(0x400530c4, ptr as u32);
    w(0x400530c8, ((N as u32) << 16) | 1);
    // Fixed source, incrementing destination, halfword width, channel IRQ.
    w(0x400530dc, 0x1104);
    w(0x4001081c, 0x105);
    w(0x4005300c, r(0x4005300c) & !(4 | (4 << 16)));
    w(0x40053010, (r(0x40053010) & !4) | (4 << 16));
    w(0x40052008, 0);
    pd_capture::write16(0x40015400, 0);
    pd_capture::write16(0x40015404, 0x98);
    fence(Ordering::SeqCst);
    w(0x4005301c, r(0x4005301c) | 4);
    pd_capture::write8(0x40015481, 0x10);
    pd_capture::write16(0x4004a000, pd_capture::read16(0x4004a000) | 0x60);
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}
/// Rearm immediately after the final TX interval, before task/JSON scheduling.
/// TX exclusively owns the stopped timer here; pool ownership is unchanged.
#[cfg(feature = "pd-tx-test")]
pub fn resume_after_tx() {
    STATE.lock(|cell| {
        if let Some(s) = cell.borrow_mut().as_mut() {
            s.running = true;
            arm(s.pool.active());
        }
    });
}
/// Timer overflow, DMA completion or DMA fault dispatch from their own IRQs.
/// This disables producers, settles DMA, validates length, changes ownership and
/// rearms before waking the decoder. It never formats text, copies or decodes.
pub fn on_interrupt(kind: u8) {
    let begin = r(0xe0001004);
    let mut wake = false;
    STATE.lock(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(s) = slot.as_mut() else {
            return;
        };
        if !s.running {
            return;
        }
        if kind == 0 && pd_capture::read8(0x40015481) & 0x40 == 0 {
            return;
        }
        if kind == 1 && r(0x40053008) & 4 == 0 {
            return;
        }
        if kind == 2 && r(0x40053004) & (4 | (4 << 16)) == 0 {
            return;
        }
        pd_capture::stop_events();
        if !stop_dma() {
            s.running = false;
            s.stats.stop_faults += 1;
            wake = true;
            return;
        }
        let count = N.saturating_sub((r(0x400530e8) >> 16) as usize);
        let events = (r(0x40052008) & 0xffff) as usize;
        s.stats.frames = s.stats.frames.wrapping_add(1);
        let mut error = kind;
        if kind == 1 {
            s.stats.full += 1;
        }
        if kind == 2 {
            s.stats.dma_errors += 1;
        }
        if events != count {
            error = 3;
            s.stats.mismatches += 1;
        }
        // Match the native minimum (>0x13 events): isolated edges on an
        // unplugged input do not occupy a consumer slot or flood telemetry.
        if kind == 0 && events == count && count < 20 {
            s.stats.noise += 1;
            arm(s.pool.active());
            s.stats.max_handler_cycles = s
                .stats
                .max_handler_cycles
                .max(r(0xe0001004).wrapping_sub(begin));
            return;
        }
        let meta = Meta {
            input: s.input,
            count,
            error,
            sequence: s.stats.frames,
            end_us: embassy_time::Instant::now().as_micros(),
            end_cycles: r(0xe0001004),
        };
        if let Some(done) = s.pool.complete() {
            s.meta[done] = meta;
            wake = true;
        } else {
            s.stats.overruns += 1;
            if s.pool.other_is_reading() {
                s.stats.held_overruns += 1;
            }
        }
        arm(s.pool.active());
        s.stats.max_handler_cycles = s
            .stats
            .max_handler_cycles
            .max(r(0xe0001004).wrapping_sub(begin));
    });
    if wake {
        WAKER.wake();
    }
}
pub struct Buffered {
    receiver: Receiver,
}
impl Buffered {
    pub fn new(receiver: Receiver) -> Self {
        Self { receiver }
    }
    pub fn start(&mut self, input: Input) -> bool {
        self.restart(input, true)
    }
    /// Reselect a passive input without resetting cumulative loss counters.
    /// Any old queued/partial burst is discarded explicitly, never mislabeled.
    pub fn switch_input(&mut self, input: Input) -> bool {
        self.restart(input, false)
    }
    fn restart(&mut self, input: Input, reset_stats: bool) -> bool {
        if !self.stop() {
            return false;
        }
        self.receiver.select_input(input);
        // DWT measures handler/rearm work, not latency before interrupt entry.
        w(0xe000edfc, r(0xe000edfc) | (1 << 24));
        w(0xe0001000, r(0xe0001000) | 1);
        STATE.lock(|cell| {
            let mut state = cell.borrow_mut();
            let mut stats = Stats::default();
            if !reset_stats && let Some(old) = state.as_ref() {
                stats = old.stats;
                stats.input_switches = stats.input_switches.wrapping_add(1);
                stats.switch_discards = stats.switch_discards.wrapping_add(
                    u32::from(old.pool.ready()) + u32::from((r(0x400530e8) >> 16) < N as u32),
                );
            }
            let s = State {
                input: input.number(),
                stats,
                running: true,
                ..State::default()
            };
            *state = Some(s);
            ENABLED.store(true, Ordering::Release);
            arm(0);
        });
        true
    }
    pub fn stop(&mut self) -> bool {
        STATE.lock(|cell| {
            pd_capture::stop_events();
            let quiet = stop_dma();
            if let Some(s) = cell.borrow_mut().as_mut() {
                s.running = false;
                if !quiet {
                    s.stats.stop_faults += 1;
                }
            }
            // Keep buffered dispatch while owning this unit; no borrowed HAL transfer
            // exists concurrently. Clear pending channel flags before returning.
            w(0x40053014, 4 | (4 << 16));
            w(0x40053018, 4 | (4 << 16));
            quiet
        })
    }
    pub fn stats(&self) -> Stats {
        STATE.lock(|c| c.borrow().as_ref().map(|s| s.stats).unwrap_or_default())
    }
    pub async fn wait_ready(&self) {
        poll_fn(|cx| {
            WAKER.register(cx.waker());
            STATE.lock(|c| {
                if c.borrow()
                    .as_ref()
                    .is_some_and(|s| s.pool.ready() || !s.running)
                {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
    pub fn frame(&mut self) -> Option<Frame<'_>> {
        STATE.lock(|c| {
            let mut state = c.borrow_mut();
            let s = state.as_mut()?;
            let slot = s.pool.acquire()?;
            fence(Ordering::SeqCst);
            Some(Frame {
                slot,
                meta: s.meta[slot],
                _owner: PhantomData,
            })
        })
    }
    pub fn faulted(&self) -> bool {
        STATE.lock(|c| c.borrow().as_ref().is_some_and(|s| !s.running))
    }
    pub async fn once(&mut self, buffer: &mut [u16]) -> Result<usize, pd_capture::Error> {
        if !self.stop() {
            return Err(pd_capture::Error::Dma(embassy_hc32::dma::Error::NotStopped));
        }
        ENABLED.store(false, Ordering::Release);
        self.receiver
            .capture(buffer, embassy_time::Duration::from_secs(15))
            .await
    }
    pub fn input(&mut self, input: Input) {
        self.receiver.select_input(input);
    }
    pub fn diagnostics(&self) -> [u32; 12] {
        self.receiver.diagnostics()
    }
}
impl Drop for Buffered {
    fn drop(&mut self) {
        self.stop();
        ENABLED.store(false, Ordering::Release);
    }
}
pub struct Frame<'a> {
    slot: usize,
    pub meta: Meta,
    _owner: PhantomData<&'a mut Buffered>,
}
impl Frame<'_> {
    pub fn samples(&self) -> &[u16] {
        // DMA cannot select a READING slot. The borrow of Buffered prevents stop/start
        // resetting the pool while this guard or its returned slice remains alive.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::addr_of!((*BUFFERS.0.get())[self.slot]).cast::<u16>(),
                self.meta.count,
            )
        }
    }
}
impl Drop for Frame<'_> {
    fn drop(&mut self) {
        STATE.lock(|c| {
            if let Some(s) = c.borrow_mut().as_mut() {
                s.pool.release(self.slot);
            }
        });
    }
}
