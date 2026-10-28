//! Generate TimerA PERAR values without touching hardware. This reproduces the
//! installed stock 2.0.7 RAM encoder, including preamble and alignment padding.
//! Reload values are not measured edge intervals: the timer counts inclusively.
use super::{DATA_SYMBOLS, ORDERED, Sop, crc32};

#[derive(Clone, Copy, Debug)]
pub struct Periods {
    pub short_reload: u16,
    pub long_reload: u16,
}
impl Periods {
    pub const NATIVE_207: Self = Self {
        short_reload: 19,
        long_reload: 38,
    };
}

#[derive(Clone, Copy, Debug)]
pub enum Frame<'a> {
    /// Header and data objects, without CRC, SOP, EOP or preamble.
    Packet {
        sop: Sop,
        wire: &'a [u8],
    },
    HardReset,
    CableReset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    Timing,
    BufferTooSmall,
}

/// On error, output remains untouched. Packets use up to seven data objects;
/// extended messages must already be split into their individual wire chunks.
/// Success establishes a waveform buffer, not permission to enable CC outputs.
pub fn encode(frame: Frame<'_>, periods: Periods, output: &mut [u16]) -> Result<usize, Error> {
    if periods.short_reload == 0 || periods.long_reload <= periods.short_reload {
        return Err(Error::Timing);
    }
    let (selector, wire) = match frame {
        Frame::Packet { sop, wire } => {
            if wire.len() < 2 || wire.len() > 30 {
                return Err(Error::Length);
            }
            let header = u16::from_le_bytes([wire[0], wire[1]]);
            if wire.len() != 2 + 4 * usize::from((header >> 12) & 7) {
                return Err(Error::Length);
            }
            let selector = match sop {
                Sop::Sop => 0,
                Sop::Prime => 1,
                Sop::DoublePrime => 2,
                Sop::PrimeDebug => 3,
                Sop::DoublePrimeDebug => 4,
            };
            (selector, wire)
        }
        Frame::HardReset => (5, &[][..]),
        Frame::CableReset => (6, &[][..]),
    };
    let crc = crc32(wire).to_le_bytes();
    let symbols = || {
        wire.iter().chain(crc.iter()).flat_map(|&b| {
            [
                DATA_SYMBOLS[usize::from(b & 15)],
                DATA_SYMBOLS[usize::from(b >> 4)],
            ]
        })
    };
    let symbol_len = |s: u8| 5 + s.count_ones() as usize;
    // The native preamble is 32 repetitions of the BMC bits 0,1: 96 reloads.
    let mut count = 96
        + ORDERED[selector]
            .iter()
            .map(|&s| symbol_len(s))
            .sum::<usize>();
    if selector < 5 {
        count += symbols().map(symbol_len).sum::<usize>() + 8; // EOP=0x0d.
        // Native DMA gets an even halfword count for ordinary packets only.
        count += count % 2;
    }
    if output.len() < count {
        return Err(Error::BufferTooSmall);
    }
    struct Writer<'a> {
        output: &'a mut [u16],
        used: usize,
        periods: Periods,
    }
    impl Writer<'_> {
        fn push(&mut self, value: u16) {
            self.output[self.used] = value;
            self.used += 1;
        }
        fn symbol(&mut self, value: u8) {
            for bit in 0..5 {
                if value & (1 << bit) != 0 {
                    self.push(self.periods.short_reload);
                    self.push(self.periods.short_reload);
                } else {
                    self.push(self.periods.long_reload);
                }
            }
        }
    }
    let mut w = Writer {
        output,
        used: 0,
        periods,
    };
    for _ in 0..32 {
        w.push(periods.long_reload);
        w.push(periods.short_reload);
        w.push(periods.short_reload);
    }
    for s in ORDERED[selector] {
        w.symbol(s);
    }
    if selector < 5 {
        for s in symbols() {
            w.symbol(s);
        }
        w.symbol(0x0d);
        if !w.used.is_multiple_of(2) {
            w.push(periods.long_reload);
        }
    }
    debug_assert_eq!(w.used, count);
    Ok(count)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::pd::{Decoder, Event};
    use std::vec::Vec;
    #[test]
    fn matches_actual_stock_207_encoder_and_decodes() {
        let mut vectors = 0;
        for line in include_str!("../../tests/data/native-pd207.tsv")
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let selector: usize = fields[0].parse().unwrap();
            let wire: Vec<_> = if fields[1] == "-" {
                Vec::new()
            } else {
                fields[1]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| u8::from_str_radix(core::str::from_utf8(s).unwrap(), 16).unwrap())
                    .collect()
            };
            let sop = [
                Sop::Sop,
                Sop::Prime,
                Sop::DoublePrime,
                Sop::PrimeDebug,
                Sop::DoublePrimeDebug,
            ][selector.min(4)];
            let frame = match selector {
                5 => Frame::HardReset,
                6 => Frame::CableReset,
                _ => Frame::Packet { sop, wire: &wire },
            };
            let expected: Vec<u16> = fields[2].split(',').map(|s| s.parse().unwrap()).collect();
            let mut output = [0; 860];
            let count = encode(frame, Periods::NATIVE_207, &mut output).unwrap();
            assert_eq!(&output[..count], expected.as_slice(), "vector {vectors}");
            // Cross-check the emitted reload sequence with the independent RX
            // decoder, accounting for the inclusive TimerA period counter.
            let mut decoder = Decoder::new(20, 5).unwrap();
            let mut event = None;
            for &reload in &output[..count] {
                if let Some(e) = decoder.push_interval(reload + 1).unwrap() {
                    assert!(event.is_none());
                    event = Some(e);
                }
            }
            decoder.finish().unwrap();
            match (frame, event.unwrap()) {
                (Frame::Packet { sop, wire }, Event::Packet(p)) => {
                    assert_eq!(p.sop, sop);
                    assert_eq!(p.wire(), wire);
                }
                (Frame::HardReset, Event::HardReset) | (Frame::CableReset, Event::CableReset) => (),
                _ => panic!("wrong decoded event"),
            }
            vectors += 1;
        }
        assert_eq!(vectors, 162);
    }
    #[test]
    fn errors_do_not_expose_partial_waveform() {
        let mut out = [0xa55a; 8];
        assert_eq!(
            encode(
                Frame::Packet {
                    sop: Sop::Sop,
                    wire: &[0x81]
                },
                Periods::NATIVE_207,
                &mut out
            ),
            Err(Error::Length)
        );
        assert_eq!(
            encode(
                Frame::Packet {
                    sop: Sop::Sop,
                    wire: &[0x81, 0x10]
                },
                Periods::NATIVE_207,
                &mut out
            ),
            Err(Error::Length)
        );
        assert_eq!(
            encode(Frame::HardReset, Periods::NATIVE_207, &mut out),
            Err(Error::BufferTooSmall)
        );
        assert_eq!(
            encode(
                Frame::HardReset,
                Periods {
                    short_reload: 0,
                    long_reload: 38
                },
                &mut out
            ),
            Err(Error::Timing)
        );
        assert_eq!(out, [0xa55a; 8]);
    }
}
