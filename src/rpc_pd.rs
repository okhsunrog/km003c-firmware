//! High-priority PD owner. Only bounded copies enter the USB reporting queue.
use crate::{
    pd_buffered::{Buffered, Meta},
    pd_capture::{Input, Receiver},
};
use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use km003c_interface::{ControlError, ControlResult, PdControl, PdEvent, PdState};
pub static EVENTS: Channel<CriticalSectionRawMutex, PdEvent, 32> = Channel::new();
static BUSY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static COMMANDS: Channel<CriticalSectionRawMutex, PdControl, 4> = Channel::new();
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
    match command {
        PdControl::Sink { volts, seconds }
            if ![5, 9, 12, 15, 20].contains(&volts) || !(1..=600).contains(&seconds) =>
        {
            return Err(ControlError::Invalid);
        }
        PdControl::Monitor { input: Some(n) } if n != 1 && n != 2 => {
            return Err(ControlError::Invalid);
        }
        PdControl::Stop => {
            crate::pd_tx::release();
        }
        _ => {}
    }
    if !matches!(command, PdControl::Stop) && BUSY.swap(true, core::sync::atomic::Ordering::AcqRel)
    {
        return Err(ControlError::Busy);
    }
    if COMMANDS.try_send(command).is_err() {
        if !matches!(command, PdControl::Stop) {
            BUSY.store(false, core::sync::atomic::Ordering::Release);
        }
        return Err(ControlError::Busy);
    }
    Ok(())
}
pub fn state(state: PdState, value: u32) {
    let _ = EVENTS.try_send(PdEvent::State { state, value });
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
        let _ = EVENTS.try_send(PdEvent::Rx {
            sequence: meta.sequence,
            input: meta.input,
            end_us: meta.end_us,
            sop: p.sop as u8,
            bytes,
            len: wire.len() as u8,
        });
    }
}
pub fn sink_tx(data: &[u8]) {
    let mut bytes = [0; 30];
    bytes[..data.len()].copy_from_slice(data);
    let (rx_to_tx_cycles, decode_cycles) = crate::pd_sink::timing();
    let _ = EVENTS.try_send(PdEvent::Tx {
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
            state(PdState::Failed, 0);
            return;
        }
    };
    let mut rx = Buffered::new(receiver);
    loop {
        let command = COMMANDS.receive().await;
        match command {
            PdControl::Stop => {
                if crate::pd_tx::release_and_wait().await.is_err() || !rx.stop() {
                    state(PdState::Failed, 0);
                }
                continue;
            }
            PdControl::Sink { volts, seconds } => {
                let _ = select(
                    COMMANDS.receive(),
                    embassy_time::with_timeout(
                        embassy_time::Duration::from_secs(u64::from(seconds)),
                        crate::pd_session::run(&mut rx, volts, seconds),
                    ),
                )
                .await;
            }
            PdControl::Monitor { input } => {
                let _ = select(COMMANDS.receive(), monitor(&mut rx, input)).await;
            }
        }
        let released = crate::pd_tx::release_and_wait().await;
        let stopped = rx.stop();
        BUSY.store(false, core::sync::atomic::Ordering::Release);
        let s = rx.stats();
        let _ = EVENTS.try_send(PdEvent::Stats {
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
