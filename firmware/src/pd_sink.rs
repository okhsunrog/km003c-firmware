//! Selected-CC software PHY adapter for the crates.io usbpd sink stack.
//! The stack owns GoodCRC, IDs, retries, reset handling and power negotiation.
use crate::{pd_buffered::Buffered, pd_service as telemetry, pd_tx};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use km003c_drivers::pd::{Event, Sop, tx::Frame};
use km003c_drivers::pd_power::fixed_request;
use km003c_interface::{FixedProfile, ProfileError};
use usbpd::{
    protocol_layer::message::data::{
        request::PowerSource, source_capabilities::SourceCapabilities,
    },
    protocol_layer::message::header::{ControlMessageType, DataMessageType, Header},
    sink::{
        device_policy_manager::{DevicePolicyManager, Event as PolicyEvent},
        policy_engine::Sink,
    },
};
use usbpd_traits::{Driver, DriverRxError, DriverTxError};

static RX_END: AtomicU32 = AtomicU32::new(0);
static DECODE_CYCLES: AtomicU32 = AtomicU32::new(0);
static REQUEST_REJECTED: AtomicBool = AtomicBool::new(false);
static PHASE_CYCLES: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];

pub fn phases() -> [u32; 4] {
    core::array::from_fn(|i| PHASE_CYCLES[i].load(Ordering::Relaxed))
}

pub fn timing() -> (u32, u32) {
    (
        pd_tx::start_cycles().wrapping_sub(RX_END.load(Ordering::Relaxed)),
        DECODE_CYCLES.load(Ordering::Relaxed),
    )
}

struct Phy<'a> {
    rx: &'a mut Buffered,
}

/// Copy the CPU-only hot loop into zero-wait SRAMH. DMA storage and application
/// data stay in their existing SRAM12 region; no loader/retention RAM is used.
pub fn init() {
    unsafe extern "C" {
        static __sipd_fast: u32;
        static mut __spd_fast: u32;
        static __epd_fast: u32;
    }
    unsafe {
        let start = core::ptr::addr_of_mut!(__spd_fast);
        let words = (core::ptr::addr_of!(__epd_fast) as usize - start as usize) / 4;
        core::ptr::copy_nonoverlapping(core::ptr::addr_of!(__sipd_fast), start, words);
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
}

// SRAMH is the fast code bank also used by native RAM routines. Ordinary SRAM
// has CPU wait states and shares instruction/data traffic, so it was slower.
#[unsafe(link_section = ".pd_fast")]
#[inline(never)]
fn decode(data: &[u16]) -> Option<Event> {
    let mut stamp = [0u32; 5];
    let result = crate::pd_fast::decode_observed(&data[data.len().min(1)..], &mut stamp);
    if matches!(result, Ok(Some(Event::Packet(_)))) {
        for i in 0..4 {
            PHASE_CYCLES[i].store(stamp[i + 1].wrapping_sub(stamp[i]), Ordering::Relaxed);
        }
    }
    result.ok().flatten()
}

impl Driver for Phy<'_> {
    const HAS_AUTO_GOOD_CRC: bool = false;
    const HAS_AUTO_RETRY: bool = false;

    async fn wait_for_vbus(&mut self) {
        loop {
            if let crate::measurement::State::Sample(s) = crate::measurement::snapshot()
                && s.bus_nanovolts() >= 4_000_000_000
            {
                return;
            }
            embassy_time::Timer::after_millis(5).await;
        }
    }

    async fn receive(&mut self, bytes: &mut [u8]) -> Result<usize, DriverRxError> {
        loop {
            let Some(frame) = self.rx.frame() else {
                self.rx.wait_ready().await;
                if self.rx.faulted() || !pd_tx::attached() {
                    return Err(DriverRxError::HardReset);
                }
                continue;
            };
            let begin = unsafe { (0xe0001004 as *const u32).read_volatile() };
            let received = if frame.meta.error == 0 {
                decode(frame.samples())
            } else {
                None
            };
            let meta = frame.meta;
            drop(frame); // Release the slot before software GoodCRC borrows TX.
            if let Some(event) = received {
                telemetry::sink_rx(event, meta);
                match event {
                    Event::Packet(p) if p.sop == Sop::Sop => {
                        let header = Header(p.header());
                        if !header.extended()
                            && header.num_objects() == 0
                            && header.message_type_raw() == ControlMessageType::Reject as u8
                        {
                            REQUEST_REJECTED.store(true, Ordering::Release);
                        }
                        RX_END.store(meta.end_cycles, Ordering::Relaxed);
                        DECODE_CYCLES.store(
                            unsafe { (0xe0001004 as *const u32).read_volatile() }
                                .wrapping_sub(begin),
                            Ordering::Relaxed,
                        );
                        if bytes.len() < p.wire().len() {
                            return Err(DriverRxError::Discarded);
                        }
                        bytes[..p.wire().len()].copy_from_slice(p.wire());
                        return Ok(p.wire().len());
                    }
                    Event::HardReset => return Err(DriverRxError::HardReset),
                    // Cable SOP' remains visible in diagnostics; never give it
                    // to the port partner's SOP protocol state/message counter.
                    _ => (),
                }
            }
        }
    }

    async fn transmit(&mut self, data: &[u8]) -> Result<(), DriverTxError> {
        if data.len() >= 2 {
            let header = Header(u16::from_le_bytes([data[0], data[1]]));
            if !header.extended()
                && header.num_objects() > 0
                && header.message_type_raw() == DataMessageType::Request as u8
            {
                REQUEST_REJECTED.store(false, Ordering::Release);
            }
        }
        let result = pd_tx::transmit(
            self.rx,
            Frame::Packet {
                sop: Sop::Sop,
                wire: data,
            },
        )
        .await;
        if result.is_err() {
            // usbpd retries Discarded in a loop. Always yield on a hardware
            // refusal so stop/expiry cannot become a synchronous retry spin.
            embassy_time::Timer::after_millis(1).await;
            return Err(if pd_tx::attached() {
                DriverTxError::Discarded
            } else {
                DriverTxError::HardReset
            });
        }
        telemetry::sink_tx(data);
        Ok(())
    }

    async fn transmit_hard_reset(&mut self) -> Result<(), DriverTxError> {
        if pd_tx::transmit(self.rx, Frame::HardReset).await.is_err() {
            embassy_time::Timer::after_millis(1).await;
            return Err(if pd_tx::attached() {
                DriverTxError::Discarded
            } else {
                DriverTxError::HardReset
            });
        }
        telemetry::sink_status("hard_reset", 5000);
        Ok(())
    }
}

struct Clock;

impl usbpd::timers::Timer for Clock {
    async fn after_millis(milliseconds: u64) {
        // now() is rounded down to 1/32768 s. One extra tick prevents an
        // elapsed wait shorter than the requested milliseconds at any phase.
        embassy_time::Timer::after(
            embassy_time::Duration::from_millis(milliseconds)
                + embassy_time::Duration::from_ticks(1),
        )
        .await;
    }
}

struct Policy {
    desired: FixedProfile,
    active: Option<FixedProfile>,
    pending_profile: Option<u32>,
}

impl DevicePolicyManager for Policy {
    async fn request(&mut self, caps: &SourceCapabilities) -> PowerSource {
        telemetry::source_profiles(caps.pdos());
        telemetry::transition_started();
        let request = fixed_request(
            caps.pdos(),
            u32::from(self.desired.volts) * 1000,
            self.desired.current_ma,
        );
        match request {
            Some(r) => {
                telemetry::sink_status("request", u32::from(self.desired.volts) * 1000);
                r
            }
            None => {
                if let Some(id) = self.pending_profile.take() {
                    telemetry::profile_finished(id, Err(ProfileError::Unsupported));
                }
                telemetry::sink_status(
                    "unsupported_using_5v",
                    u32::from(self.desired.volts) * 1000,
                );
                self.desired = FixedProfile {
                    volts: 5,
                    current_ma: 100,
                };
                // A malformed source must not turn a failed selection into a
                // panic or an unadvertised request.
                match fixed_request(caps.pdos(), 5000, 100) {
                    Some(r) => r,
                    None => core::future::pending().await,
                }
            }
        }
    }

    async fn transition_power(&mut self, _: &PowerSource) {
        // Called by usbpd after Accept + PS_RDY, not merely after sending RDO.
        self.active = Some(self.desired);
        telemetry::contract_ready(self.desired);
        if let Some(id) = self.pending_profile.take() {
            telemetry::profile_finished(id, Ok(self.desired));
        }
    }

    async fn get_event(&mut self, caps: &SourceCapabilities) -> PolicyEvent {
        // usbpd 2 calls this after both Reject and Wait. Wait keeps the request
        // pending: the PE retries after tSinkRequest, without a new host command.
        // Only an actual Reject completes the command as rejected.
        if REQUEST_REJECTED.swap(false, Ordering::AcqRel)
            && let Some(id) = self.pending_profile.take()
        {
            self.desired = self.active.unwrap_or(FixedProfile {
                volts: 5,
                current_ma: 100,
            });
            if let Some(active) = self.active {
                telemetry::contract_ready(active);
            }
            telemetry::profile_finished(id, Err(ProfileError::Rejected));
        }
        if self.pending_profile.is_some() {
            return core::future::pending().await;
        }
        loop {
            let r = telemetry::next_profile().await;
            let Some(request) = fixed_request(
                caps.pdos(),
                u32::from(r.profile.volts) * 1000,
                r.profile.current_ma,
            ) else {
                telemetry::profile_finished(r.id, Err(ProfileError::Unsupported));
                continue;
            };
            telemetry::profile_start(r.id);
            telemetry::transition_started();
            self.desired = r.profile;
            self.pending_profile = Some(r.id);
            telemetry::sink_status("request", u32::from(r.profile.volts) * 1000);
            return PolicyEvent::RequestPower(request);
        }
    }

    async fn hard_reset(&mut self) {
        self.active = None;
        telemetry::transition_started();
        if let Some(id) = self.pending_profile.take() {
            telemetry::profile_finished(id, Err(ProfileError::Cancelled));
        }
        self.desired = telemetry::snapshot()
            .selected_profile
            .unwrap_or(self.desired);
        telemetry::sink_status("transition_default", 5000);
    }
}

pub async fn run(rx: &mut Buffered, profile: FixedProfile) -> Result<(), ()> {
    let mut sink = Sink::<_, Clock, _>::new(
        Phy { rx },
        Policy {
            desired: profile,
            active: None,
            pending_profile: None,
        },
    );
    if let Err(error) = sink.run().await {
        log::warn!("PD sink stopped: {error:?}");
        return Err(());
    }
    Ok(())
}
