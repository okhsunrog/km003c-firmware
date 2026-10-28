//! Allocation-free measurement and passive PD decoding, independent of HC32 MMIO.
#![no_std]

pub mod ina229;
pub mod pd;
pub mod tile;
