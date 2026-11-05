#![no_std]
//! Shared wire contract. Physical units are integers; no device-side JSON or heap.
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 4;
pub const DEFAULT_STREAM_INTERVAL_MS: u16 = 250;
pub const MIN_STREAM_INTERVAL_MS: u16 = 20;
pub const MAX_STREAM_INTERVAL_MS: u16 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct FixedProfile {
    pub volts: u16,
    pub current_ma: u16,
}

impl FixedProfile {
    /// SPR only: no cable-current discovery is implemented for >3 A operation.
    pub const fn valid(self) -> bool {
        matches!(self.volts, 5 | 9 | 12 | 15 | 20)
            && self.current_ma >= 100
            && self.current_ma <= 3000
            && self.current_ma.is_multiple_of(10)
    }
}

pub const fn valid_stream_interval(ms: u16) -> bool {
    ms >= MIN_STREAM_INTERVAL_MS && ms <= MAX_STREAM_INTERVAL_MS
}

pub const fn default_current_ma() -> u16 {
    100
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub struct Status {
    pub protocol_version: u16,
    pub uptime_ms: u64,
    pub hclk_hz: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub struct Measurement {
    pub uptime_ms: u64,
    /// Time and sequence of the fresh INA229 register acquisition, not USB send.
    pub sampled_at_ms: u64,
    pub sample_sequence: u32,
    pub voltage_uv: u32,
    pub current_ua: Option<i32>,
    pub temperature_mc: i32,
    pub calibrated: bool,
    pub diagnostic: u16,
    /// CC1, CC2, D+, D-. None means no valid ADC snapshot.
    pub lines_uv: Option<[u32; 4]>,
}

pub type LoaderResult = Result<(), ()>;
pub type MeasurementResult = Option<Measurement>;
pub type StreamIntervalResult = Result<(), ControlError>;
postcard_rpc::endpoints! {
    list = ENDPOINTS;
    | EndpointTy | RequestTy | ResponseTy | Path |
    | ---------- | --------- | ---------- | ---- |
    | GetStatus | () | Status | "km003c/status" |
    | GetPdStatus | () | PdSnapshot | "km003c/pd/status" |
    | GetMeasurement | () | MeasurementResult | "km003c/measurement" |
    | SetStreamInterval | u16 | StreamIntervalResult | "km003c/stream/interval" |
    | SetStream | bool | () | "km003c/stream" |
    | EnterLoader | () | LoaderResult | "km003c/loader" |
    | ControlPd | PdControl | ControlResult | "km003c/pd/control" |
}
postcard_rpc::topics! {
    list = TOPICS_IN;
    direction = postcard_rpc::TopicDirection::ToServer;
    | TopicTy | MessageTy | Path |
    | ------- | --------- | ---- |
}
postcard_rpc::topics! {
    list = TOPICS_OUT;
    direction = postcard_rpc::TopicDirection::ToClient;
    | TopicTy | MessageTy | Path |
    | ------- | --------- | ---- |
    | Measurements | Measurement | "km003c/measurements" |
    | PdEvents | PdEvent | "km003c/pd/events" |
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub enum PdControl {
    Stop,
    /// Passive reception; None follows the active CC selected by ADC.
    Monitor {
        input: Option<u8>,
    },
    Sink {
        volts: u16,
        seconds: u16,
        #[serde(default = "default_current_ma")]
        current_ma: u16,
    },
    /// Renegotiate within the active sink session without releasing Rd.
    SetProfile {
        volts: u16,
        current_ma: u16,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub enum ControlError {
    Invalid,
    Busy,
    Unavailable,
}

/// Accepted command ID; terminal events carry this ID.
pub type ControlResult = Result<u32, ControlError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub enum PdEvent {
    Stats {
        counters: [u32; 12],
    },
    Rx {
        sequence: u32,
        input: u8,
        end_us: u64,
        sop: u8,
        bytes: [u8; 30],
        len: u8,
    },
    Reset {
        sequence: u32,
        input: u8,
        end_us: u64,
        kind: ResetKind,
    },
    Tx {
        bytes: [u8; 30],
        len: u8,
        rx_to_tx_cycles: u32,
        decode_cycles: u32,
        phases: [u32; 4],
    },
    State {
        command_id: u32,
        state: PdState,
        value: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum ResetKind {
    Hard,
    Cable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum PdState {
    Starting,
    Request,
    Ready,
    Detached,
    HardReset,
    Default,
    Fallback,
    Waiting,
    Attached,
    Released,
    Reorienting,
    Failed,
}

/// Terminal reason and cleanup are independent: failure can still detach safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum PdEndReason {
    Stopped,
    TimedOut,
    Completed,
    RxStartFailed,
    AttachFailed,
    ReleaseFailed,
    RxStopFailed,
    StackFailed,
    MonitorError,
    InitializationError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct PdCompletion {
    pub command_id: u32,
    pub reason: PdEndReason,
    pub released: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct PdSnapshot {
    pub available: bool,
    pub state: PdState,
    /// Accepted non-Stop command, including one still queued for the owner.
    pub active_command: Option<u32>,
    /// Most recently finished sink/monitor, retained when Stop completes.
    pub last_session: Option<PdCompletion>,
    /// Most recently completed command, including Stop.
    pub last_command: Option<PdCompletion>,
    /// Present only after Accept + PS_RDY; cleared before a new transition.
    pub contract: Option<FixedProfile>,
    pub selected_profile: Option<FixedProfile>,
    pub pending_profile: Option<u32>,
    pub last_profile: Option<ProfileCompletion>,
    /// Fixed PDOs from the latest source capabilities, in advertised order.
    pub source_profiles: [Option<FixedOffer>; 7],
    /// Cumulative since boot; these do not include decoder/DMA capture errors.
    pub event_queue_drops: u32,
    pub event_publish_failures: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct FixedOffer {
    pub position: u8,
    pub millivolts: u32,
    pub max_current_ma: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ProfileCompletion {
    pub command_id: u32,
    pub result: Result<FixedProfile, ProfileError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum ProfileError {
    Unsupported,
    Rejected,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_spr_profiles_and_sampling_bounds() {
        assert!(
            FixedProfile {
                volts: 15,
                current_ma: 3000
            }
            .valid()
        );
        for p in [
            FixedProfile {
                volts: 28,
                current_ma: 3000,
            },
            FixedProfile {
                volts: 15,
                current_ma: 0,
            },
            FixedProfile {
                volts: 20,
                current_ma: 5000,
            },
        ] {
            assert!(!p.valid());
        }
        assert!(valid_stream_interval(20));
        assert!(valid_stream_interval(5000));
        assert!(!valid_stream_interval(19));
        assert!(!valid_stream_interval(5001));
    }
}
