//! USB PD PHY decoding and offline waveform encoding. No hardware transmitter,
//! GoodCRC generator or running policy engine is provided by this module.
//!
//! Input to `Decoder` is a sequence of edge-to-edge timer intervals, not GPIO
//! levels. The capture adapter must establish whether hardware supplies deltas
//! or timestamps before feeding it. Timebase/tolerance are explicit and are not
//! inferred from the native firmware's 19/38 tick transmission table.

pub mod tx;

const DATA_SYMBOLS: [u8; 16] = [
    0x1e, 0x09, 0x14, 0x15, 0x0a, 0x0b, 0x0e, 0x0f, 0x12, 0x13, 0x16, 0x17, 0x1a, 0x1b, 0x1c, 0x1d,
];
const ORDERED: [[u8; 4]; 7] = [
    [0x18, 0x18, 0x18, 0x11],
    [0x18, 0x18, 0x06, 0x06],
    [0x18, 0x06, 0x18, 0x06],
    [0x18, 0x19, 0x11, 0x06],
    [0x18, 0x19, 0x06, 0x11],
    [0x07, 0x07, 0x07, 0x19],
    [0x07, 0x18, 0x07, 0x06],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sop {
    Sop,
    Prime,
    DoublePrime,
    PrimeDebug,
    DoublePrimeDebug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub sop: Sop,
    /// Header plus up to seven 32-bit data objects, in wire byte order.
    /// Extended messages remain raw chunks; reassembly belongs above this PHY.
    pub bytes: [u8; 30],
    pub len: u8,
    pub crc: u32,
}
impl Packet {
    pub fn header(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }
    pub fn wire(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Packet(Packet),
    HardReset,
    CableReset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Timing,
    InvalidSymbol(u8),
    UnexpectedEop,
    MissingEop,
    Crc,
    Truncated,
}

/// CRC-32/ISO-HDLC: reflected polynomial, initial/final XOR all ones. USB PD
/// transmits its four result bytes least significant first. Check vector
/// "123456789" -> 0xcbf43926 is independent of our synthetic packet encoder.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub struct Decoder {
    half: u16,
    tolerance: u16,
    pending_half: bool,
    window: u32,
    seen: u8,
    sop: Option<Sop>,
    symbol: u8,
    symbol_bits: u8,
    low_nibble: Option<u8>,
    bytes: [u8; 34],
    used: usize,
    expected: usize,
}
impl Decoder {
    pub fn new(half_bit_ticks: u16, tolerance: u16) -> Option<Self> {
        // Keep short/long windows disjoint, and all arithmetic representable.
        if !(4..=u16::MAX / 2).contains(&half_bit_ticks) || tolerance > half_bit_ticks / 4 {
            return None;
        }
        Some(Self {
            half: half_bit_ticks,
            tolerance,
            pending_half: false,
            window: 0,
            seen: 0,
            sop: None,
            symbol: 0,
            symbol_bits: 0,
            low_nibble: None,
            bytes: [0; 34],
            used: 0,
            expected: 0,
        })
    }
    /// Whether a SOP has been recognized and a packet is being assembled.
    pub fn in_packet(&self) -> bool {
        self.sop.is_some()
    }
    pub fn reset(&mut self) {
        self.pending_half = false;
        self.window = 0;
        self.seen = 0;
        self.sop = None;
        self.symbol = 0;
        self.symbol_bits = 0;
        self.low_nibble = None;
        self.used = 0;
        self.expected = 0;
    }
    /// Packet-gap/overrun notification. Do not join two DMA buffers across lost
    /// edges. A missing EOP or incomplete symbol is reported, then state clears.
    pub fn finish(&mut self) -> Result<(), Error> {
        let incomplete = self.sop.is_some();
        self.reset();
        if incomplete {
            Err(Error::Truncated)
        } else {
            Ok(())
        }
    }
    pub fn push_interval(&mut self, ticks: u16) -> Result<Option<Event>, Error> {
        let n = u32::from(ticks);
        let half = u32::from(self.half);
        let tol = u32::from(self.tolerance);
        let result = if n.abs_diff(half) <= tol {
            if !self.pending_half {
                self.pending_half = true;
                return Ok(None);
            }
            self.pending_half = false;
            self.bit(true)
        } else if n.abs_diff(2 * half) <= 2 * tol && !self.pending_half {
            self.bit(false)
        } else {
            Err(Error::Timing)
        };
        if result.is_err() {
            self.reset();
        }
        result
    }
    fn bit(&mut self, bit: bool) -> Result<Option<Event>, Error> {
        if self.sop.is_none() {
            // Search at every bit position, so preamble and leading noise do
            // not force an assumed 5-bit symbol boundary or cable polarity.
            self.window = (self.window >> 1) | (u32::from(bit) << 19);
            self.seen = self.seen.saturating_add(1).min(20);
            if self.seen < 20 {
                return Ok(None);
            }
            for (i, set) in ORDERED.iter().enumerate() {
                let packed = set
                    .iter()
                    .enumerate()
                    .fold(0, |n, (i, s)| n | (u32::from(*s) << (5 * i)));
                if self.window != packed {
                    continue;
                }
                if i >= 5 {
                    let event = if i == 5 {
                        Event::HardReset
                    } else {
                        Event::CableReset
                    };
                    self.reset();
                    return Ok(Some(event));
                }
                self.sop = Some(
                    [
                        Sop::Sop,
                        Sop::Prime,
                        Sop::DoublePrime,
                        Sop::PrimeDebug,
                        Sop::DoublePrimeDebug,
                    ][i],
                );
                return Ok(None);
            }
            return Ok(None);
        }
        // USB PD sends each 5-bit symbol LSB first, low nibble before high.
        self.symbol |= u8::from(bit) << self.symbol_bits;
        self.symbol_bits += 1;
        if self.symbol_bits != 5 {
            return Ok(None);
        }
        let symbol = self.symbol;
        self.symbol = 0;
        self.symbol_bits = 0;
        if symbol == 0x0d {
            if self.expected == 0 || self.used != self.expected || self.low_nibble.is_some() {
                return Err(Error::UnexpectedEop);
            }
            let size = self.used - 4;
            let crc = u32::from_le_bytes(self.bytes[size..self.used].try_into().unwrap());
            if crc32(&self.bytes[..size]) != crc {
                return Err(Error::Crc);
            }
            let mut packet = Packet {
                sop: self.sop.unwrap(),
                bytes: [0; 30],
                len: size as u8,
                crc,
            };
            packet.bytes[..size].copy_from_slice(&self.bytes[..size]);
            self.reset();
            return Ok(Some(Event::Packet(packet)));
        }
        if self.expected != 0 && self.used == self.expected {
            return Err(Error::MissingEop);
        }
        let nibble = DATA_SYMBOLS
            .iter()
            .position(|&s| s == symbol)
            .ok_or(Error::InvalidSymbol(symbol))? as u8;
        if let Some(low) = self.low_nibble.take() {
            // Header NDO bounds the whole frame to 34 bytes including CRC.
            self.bytes[self.used] = low | (nibble << 4);
            self.used += 1;
            if self.used == 2 {
                self.expected = 6 + 4 * usize::from((self.bytes[1] >> 4) & 7);
            }
        } else {
            self.low_nibble = Some(nibble);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
