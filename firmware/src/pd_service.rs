//! High-priority PD owner. Only bounded copies enter the USB reporting queue.
use crate::{
    pd_buffered::{Buffered, Meta},
    pd_capture::{Input, Receiver},
};
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use km003c_interface::{
    ControlError, ControlResult, FixedOffer, FixedProfile, PdCompletion, PdControl, PdEndReason,
    PdEvent, PdSnapshot, PdState, ProfileCompletion, ProfileError, ResetKind,
};

pub static EVENTS: Channel<CriticalSectionRawMutex, PdEvent, 32> = Channel::new();
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
// OWNER reserves a queued/running capture. Stop cannot clear a newer reservation.
static OWNER: AtomicU32 = AtomicU32::new(0);
static NEXT: AtomicU32 = AtomicU32::new(1);
static CURRENT: AtomicU32 = AtomicU32::new(0);
static PROFILE_ALLOWED: AtomicBool = AtomicBool::new(false);
// Stop must win even if the high-priority DPM finishes a transition while the
// USB handler is releasing the CC hardware.
static STOPPING: AtomicBool = AtomicBool::new(false);
static EVENT_QUEUE_DROPS: AtomicU32 = AtomicU32::new(0);
static EVENT_PUBLISH_FAILURES: AtomicU32 = AtomicU32::new(0);

fn event(event: PdEvent) {
    if EVENTS.try_send(event).is_err() {
        EVENT_QUEUE_DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn publish_failed() {
    EVENT_PUBLISH_FAILURES.fetch_add(1, Ordering::Relaxed);
}

pub struct ProfileRequest {
    pub id: u32,
    pub profile: FixedProfile,
    owner: u32,
}
static PROFILES: Channel<CriticalSectionRawMutex, ProfileRequest, 1> = Channel::new();
static SNAPSHOT: embassy_sync::blocking_mutex::Mutex<
    CriticalSectionRawMutex,
    core::cell::Cell<PdSnapshot>,
> = embassy_sync::blocking_mutex::Mutex::new(core::cell::Cell::new(PdSnapshot {
    available: false,
    state: PdState::Detached,
    active_command: None,
    last_session: None,
    last_command: None,
    contract: None,
    selected_profile: None,
    pending_profile: None,
    last_profile: None,
    source_profiles: [None; 7],
    event_queue_drops: 0,
    event_publish_failures: 0,
}));

pub fn source_profiles(
    pdos: &[usbpd::protocol_layer::message::data::source_capabilities::PowerDataObject],
) {
    use usbpd::protocol_layer::message::data::source_capabilities::PowerDataObject;
    let offers = core::array::from_fn(|i| match pdos.get(i) {
        Some(PowerDataObject::FixedSupply(f)) if f.raw_voltage() > 0 => Some(FixedOffer {
            position: (i + 1) as u8,
            millivolts: u32::from(f.raw_voltage()) * 50,
            max_current_ma: f.raw_max_current() * 10,
        }),
        _ => None,
    });
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        x.source_profiles = offers;
        s.set(x);
    });
}

pub fn transition_started() {
    PROFILE_ALLOWED.store(false, Ordering::Release);
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        x.contract = None;
        s.set(x);
    });
}

pub fn contract_ready(profile: FixedProfile) {
    SNAPSHOT.lock(|s| {
        if STOPPING.load(Ordering::Acquire) {
            return;
        }
        let mut x = s.get();
        x.contract = Some(profile);
        x.selected_profile = Some(profile);
        s.set(x);
        PROFILE_ALLOWED.store(x.pending_profile.is_none(), Ordering::Release);
    });
    state(PdState::Ready, u32::from(profile.volts) * 1000);
}

pub fn profile_finished(id: u32, result: Result<FixedProfile, ProfileError>) {
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        if x.pending_profile == Some(id) {
            x.pending_profile = None;
            x.last_profile = Some(ProfileCompletion {
                command_id: id,
                result,
            });
        }
        s.set(x);
        PROFILE_ALLOWED.store(
            x.contract.is_some()
                && x.pending_profile.is_none()
                && !STOPPING.load(Ordering::Acquire),
            Ordering::Release,
        );
    });
}

pub async fn next_profile() -> ProfileRequest {
    loop {
        let r = PROFILES.receive().await;
        if r.owner == OWNER.load(Ordering::Acquire)
            && snapshot().pending_profile == Some(r.id)
            && !STOPPING.load(Ordering::Acquire)
        {
            return r;
        }
        profile_finished(r.id, Err(ProfileError::Cancelled));
    }
}

/// The DPM future may be dropped on disconnect before it consumes a command.
/// Drain it here so a completed Cancelled command cannot run after reattach.
pub fn cancel_profiles() {
    while let Ok(r) = PROFILES.try_receive() {
        profile_finished(r.id, Err(ProfileError::Cancelled));
    }
    if let Some(id) = snapshot().pending_profile {
        profile_finished(id, Err(ProfileError::Cancelled));
    }
}

pub fn profile_start(id: u32) {
    CURRENT.store(id, Ordering::Release);
}

pub fn snapshot() -> PdSnapshot {
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        let owner = OWNER.load(Ordering::Acquire);
        x.active_command = (owner != 0).then_some(owner);
        x.event_queue_drops = EVENT_QUEUE_DROPS.load(Ordering::Relaxed);
        x.event_publish_failures = EVENT_PUBLISH_FAILURES.load(Ordering::Relaxed);
        x
    })
}

fn complete(session: Option<PdCompletion>, command: PdCompletion) {
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        if session.is_some() {
            x.last_session = session;
        }
        x.last_command = Some(command);
        s.set(x);
    });
}

#[derive(Clone, Copy)]
struct Request {
    id: u32,
    command: PdControl,
}

static COMMANDS: Channel<CriticalSectionRawMutex, Request, 4> = Channel::new();
static EXECUTOR: embassy_executor::InterruptExecutor = embassy_executor::InterruptExecutor::new();
use embassy_hc32::pac::interrupt;

#[interrupt]
fn INT030() {
    unsafe { EXECUTOR.on_interrupt() };
}

pub fn start() {
    // Software-pended PD executor; timer/DMA IRQs may preempt the decoder.
    unsafe {
        (0x400510d4 as *mut u32).write_volatile(0x1ff);
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(interrupt::INT030, 0x70);
    }
    EXECUTOR.start(interrupt::INT030).spawn(task().unwrap());
}

pub fn control(command: PdControl) -> ControlResult {
    if !snapshot().available {
        return Err(ControlError::Unavailable);
    }
    match command {
        PdControl::Sink {
            volts,
            seconds,
            current_ma,
        } if !(FixedProfile { volts, current_ma }).valid() || !(1..=600).contains(&seconds) => {
            return Err(ControlError::Invalid);
        }
        PdControl::Monitor { input: Some(n) } if n != 1 && n != 2 => {
            return Err(ControlError::Invalid);
        }
        PdControl::Stop => {
            SNAPSHOT.lock(|s| {
                STOPPING.store(true, Ordering::Release);
                PROFILE_ALLOWED.store(false, Ordering::Release);
                let mut x = s.get();
                x.contract = None;
                s.set(x);
            });
            crate::pd_tx::release();
        }
        _ => {}
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    // Zero is reserved for events not belonging to a host command.
    let id = if id == 0 {
        NEXT.fetch_add(1, Ordering::Relaxed)
    } else {
        id
    };
    if let PdControl::SetProfile { volts, current_ma } = command {
        let profile = FixedProfile { volts, current_ma };
        if !profile.valid() {
            return Err(ControlError::Invalid);
        }
        let owner = SNAPSHOT.lock(|s| {
            let owner = OWNER.load(Ordering::Acquire);
            if owner == 0
                || STOPPING.load(Ordering::Acquire)
                || PROFILE_ALLOWED
                    .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
            {
                return Err(ControlError::Busy);
            }
            let mut x = s.get();
            x.pending_profile = Some(id);
            s.set(x);
            Ok(owner)
        })?;
        if PROFILES
            .try_send(ProfileRequest { id, profile, owner })
            .is_err()
        {
            profile_finished(id, Err(ProfileError::Cancelled));
            return Err(ControlError::Busy);
        }
        return Ok(id);
    }
    if !matches!(command, PdControl::Stop)
        && OWNER
            .compare_exchange(0, id, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return Err(ControlError::Busy);
    }
    if COMMANDS.try_send(Request { id, command }).is_err() {
        let _ = OWNER.compare_exchange(id, 0, Ordering::AcqRel, Ordering::Acquire);
        return Err(ControlError::Busy);
    }
    Ok(id)
}

pub fn state(state: PdState, value: u32) {
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        x.state = state;
        s.set(x);
    });
    event(PdEvent::State {
        command_id: CURRENT.load(Ordering::Acquire),
        state,
        value,
    });
}

pub fn sink_status(s: &str, value: u32) {
    state(
        match s {
            "starting" => PdState::Starting,
            "request" => PdState::Request,
            "contract_ready" => PdState::Ready,
            "hard_reset" => PdState::HardReset,
            "transition_default" => PdState::Default,
            "unsupported_using_5v" => PdState::Fallback,
            _ => PdState::Failed,
        },
        value,
    );
}

pub fn cc_state(s: &str, input: u8) {
    state(
        match s {
            "waiting" => PdState::Waiting,
            "attached" => PdState::Attached,
            "released" => PdState::Released,
            "reorienting" => PdState::Reorienting,
            _ => PdState::Failed,
        },
        u32::from(input),
    );
}

pub fn sink_rx(event: km003c_drivers::pd::Event, meta: Meta) {
    if let km003c_drivers::pd::Event::Packet(p) = event {
        let mut bytes = [0; 30];
        let wire = p.wire();
        bytes[..wire.len()].copy_from_slice(wire);
        self::event(PdEvent::Rx {
            sequence: meta.sequence,
            input: meta.input,
            end_us: meta.end_us,
            sop: p.sop as u8,
            bytes,
            len: wire.len() as u8,
        });
    } else {
        self::event(PdEvent::Reset {
            sequence: meta.sequence,
            input: meta.input,
            end_us: meta.end_us,
            kind: match event {
                km003c_drivers::pd::Event::HardReset => ResetKind::Hard,
                km003c_drivers::pd::Event::CableReset => ResetKind::Cable,
                _ => unreachable!(),
            },
        });
    }
}

pub fn sink_tx(data: &[u8]) {
    let mut bytes = [0; 30];
    bytes[..data.len()].copy_from_slice(data);
    let (rx_to_tx_cycles, decode_cycles) = crate::pd_sink::timing();
    event(PdEvent::Tx {
        bytes,
        len: data.len() as u8,
        rx_to_tx_cycles,
        decode_cycles,
        phases: crate::pd_sink::phases(),
    });
}

#[embassy_executor::task]
async fn task() {
    let receiver = match unsafe { Receiver::new(Input::Cmp1Inp1, 0x20) } {
        Ok(r) => r,
        Err(error) => {
            log::error!("PD initialization failed: {error}");
            complete(
                None,
                PdCompletion {
                    command_id: 0,
                    reason: PdEndReason::InitializationError,
                    released: false,
                },
            );
            state(PdState::Failed, 0);
            return;
        }
    };
    SNAPSHOT.lock(|s| {
        let mut x = s.get();
        x.available = true;
        s.set(x);
    });
    let mut rx = Buffered::new(receiver);
    loop {
        let request = COMMANDS.receive().await;
        CURRENT.store(request.id, Ordering::Release);
        let command = request.command;
        if !matches!(command, PdControl::Stop) {
            STOPPING.store(false, Ordering::Release);
        }
        let mut reason = PdEndReason::Completed;
        let mut stop_id = None;
        match command {
            PdControl::Stop => {
                let released = crate::pd_tx::release_and_wait().await;
                let stopped = rx.stop();
                complete(
                    None,
                    PdCompletion {
                        command_id: request.id,
                        reason: PdEndReason::Stopped,
                        released: released.is_ok() && stopped,
                    },
                );
                state(
                    if released.is_ok() && stopped {
                        PdState::Detached
                    } else {
                        PdState::Failed
                    },
                    0,
                );
                continue;
            }
            PdControl::Sink {
                volts,
                seconds,
                current_ma,
            } => {
                SNAPSHOT.lock(|s| {
                    let mut x = s.get();
                    x.selected_profile = Some(FixedProfile { volts, current_ma });
                    x.last_profile = None;
                    x.source_profiles = [None; 7];
                    s.set(x);
                });
                let outcome = select(
                    COMMANDS.receive(),
                    embassy_time::with_timeout(
                        embassy_time::Duration::from_secs(u64::from(seconds)),
                        crate::pd_session::run(
                            &mut rx,
                            FixedProfile { volts, current_ma },
                            seconds,
                        ),
                    ),
                )
                .await;
                match outcome {
                    Either::First(stop) => {
                        stop_id = Some(stop.id);
                        reason = PdEndReason::Stopped;
                        CURRENT.store(stop.id, Ordering::Release);
                    }
                    Either::Second(Err(_)) => reason = PdEndReason::TimedOut,
                    Either::Second(Ok(Err(error))) => reason = error,
                    Either::Second(Ok(Ok(()))) => {}
                }
            }
            PdControl::Monitor { input } => {
                if let Either::First(stop) =
                    select(COMMANDS.receive(), monitor(&mut rx, input)).await
                {
                    stop_id = Some(stop.id);
                    reason = PdEndReason::Stopped;
                    CURRENT.store(stop.id, Ordering::Release);
                } else {
                    reason = PdEndReason::MonitorError;
                }
            }
            PdControl::SetProfile { .. } => unreachable!("profile requests go to the active DPM"),
        }
        transition_started();
        cancel_profiles();
        let released = crate::pd_tx::release_and_wait().await;
        let stopped = rx.stop();
        let clean = released.is_ok() && stopped;
        let session = PdCompletion {
            command_id: request.id,
            reason,
            released: clean,
        };
        let command = stop_id.map_or(session, |id| PdCompletion {
            command_id: id,
            reason: PdEndReason::Stopped,
            released: clean,
        });
        // Publish completion before clearing OWNER, so idle never exposes stale results.
        SNAPSHOT.lock(|s| {
            let mut x = s.get();
            x.last_session = Some(session);
            x.last_command = Some(command);
            s.set(x);
            let _ = OWNER.compare_exchange(request.id, 0, Ordering::AcqRel, Ordering::Acquire);
        });
        let s = rx.stats();
        event(PdEvent::Stats {
            counters: [
                s.frames,
                s.overruns,
                s.held_overruns,
                s.noise,
                s.full,
                s.dma_errors,
                s.mismatches,
                s.stop_faults,
                s.input_switches,
                s.switch_discards,
                s.max_handler_cycles,
                0,
            ],
        });
        state(
            if released.is_ok() && stopped {
                PdState::Detached
            } else {
                PdState::Failed
            },
            0,
        );
    }
}

/// Passive monitoring never attaches Rd or acknowledges packets. Reconfiguration
/// happens through the capture owner's stop/start boundary, never during TX.
async fn monitor(rx: &mut Buffered, fixed: Option<u8>) {
    let mut input = fixed.or_else(crate::lines::cc_input).unwrap_or(1);
    let select_input = |n| {
        if n == 2 {
            Input::Cmp1Inp2
        } else {
            Input::Cmp1Inp1
        }
    };
    if !rx.start(select_input(input)) {
        state(PdState::Failed, 0);
        return;
    }
    loop {
        if fixed.is_none()
            && let Some(next) = crate::lines::cc_input()
            && next != input
        {
            if !rx.switch_input(select_input(next)) {
                state(PdState::Failed, 0);
                return;
            }
            input = next;
        }
        if let Some(frame) = rx.frame() {
            let meta = frame.meta;
            if meta.error == 0 {
                let mut decoder = km003c_drivers::pd::Decoder::new(20, 5).unwrap();
                for &tick in &frame.samples()[frame.samples().len().min(1)..] {
                    if let Ok(Some(event)) = decoder.push_interval(tick) {
                        sink_rx(event, meta);
                    }
                }
            }
            drop(frame);
        } else {
            if rx.faulted() {
                state(PdState::Failed, 0);
                return;
            }
            let _ = select(rx.wait_ready(), embassy_time::Timer::after_millis(20)).await;
        }
    }
}
