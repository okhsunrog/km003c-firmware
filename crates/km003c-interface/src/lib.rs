#![no_std]
//! Shared wire contract. Physical units are integers; no device-side JSON or heap.
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub struct Status {
    pub protocol_version: u16,
    pub uptime_ms: u64,
    pub hclk_hz: u32,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub struct Measurement {
    pub uptime_ms: u64,
    pub voltage_uv: u32,
    pub current_ua: Option<i32>,
    pub temperature_mc: i32,
    pub calibrated: bool,
    pub diagnostic: u16,
    /// CC1, CC2, D+, D-. None means no valid ADC snapshot.
    pub lines_uv: Option<[u32; 4]>,
}
pub type LoaderResult = Result<(), ()>;
postcard_rpc::endpoints! {
    list = ENDPOINTS;
    | EndpointTy | RequestTy | ResponseTy | Path |
    | ---------- | --------- | ---------- | ---- |
    | GetStatus | () | Status | "km003c/status" |
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
    Sink { volts: u16, seconds: u16 },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
pub enum ControlError {
    Invalid,
    Busy,
}
pub type ControlResult = Result<(), ControlError>;
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
    Tx {
        bytes: [u8; 30],
        len: u8,
        rx_to_tx_cycles: u32,
        decode_cycles: u32,
        phases: [u32; 4],
    },
    State {
        state: PdState,
        value: u32,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Schema)]
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
