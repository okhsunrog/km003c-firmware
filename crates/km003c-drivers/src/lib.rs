//! Allocation-free measurement and passive PD decoding, independent of HC32 MMIO.
#![no_std]

pub mod calibration;
pub mod ina229;
pub mod pd;
pub mod tile;

pub mod rotation;
pub mod sc7a20;
