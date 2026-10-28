//! Emit a deterministic scanner differential corpus for target emulation.
use km003c_drivers::pd::{
    Sop,
    symbols::{self, CAPACITY},
    tx::{self, Frame, Periods},
};
use std::{fmt::Write, fs};
fn main() {
    let mut output = String::new();
    let mut state = 0x3b6e20c8u32;
    let mut emit = |data: &[u16]| {
        let mut syms = [0u8; CAPACITY];
        let status = symbols::scan::<0, true>(data, &mut syms, symbols::scalar_pair);
        let count = if status & 0xc0000000 == 0 {
            (status & 255) as usize
        } else {
            0
        };
        write!(output, "{status:08x}\t").unwrap();
        for n in &syms[..count] {
            write!(output, "{n:02x}").unwrap();
        }
        output.push('\t');
        for (i, n) in data.iter().enumerate() {
            if i != 0 {
                output.push(',');
            }
            write!(output, "{n}").unwrap();
        }
        output.push('\n');
    };
    let mut reload = [0u16; 860];
    let mut wire = [0u8; 30];
    for sop in [
        Sop::Sop,
        Sop::Prime,
        Sop::DoublePrime,
        Sop::PrimeDebug,
        Sop::DoublePrimeDebug,
    ] {
        for ndo in 0..8 {
            for fill in [0, 255, 85, 170] {
                wire.fill(fill);
                wire[0] = 0x81;
                wire[1] = (ndo as u8) << 4;
                let n = tx::encode(
                    Frame::Packet {
                        sop,
                        wire: &wire[..2 + 4 * ndo],
                    },
                    Periods::NATIVE_207,
                    &mut reload,
                )
                .unwrap();
                let data: Vec<_> = reload[..n].iter().map(|n| n + 1).collect();
                emit(&data);
                let jitter: Vec<_> = data
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| (i32::from(x) + [-4, 0, 4][i % 3]) as u16)
                    .collect();
                emit(&jitter);
                let mut noise = vec![1, 1000, 20, 40, 999];
                noise.extend_from_slice(&data);
                emit(&noise);
                for cut in [0, 1, 2, 31, 63, 95, 120, n - 3, n - 1] {
                    emit(&data[..cut.min(n)]);
                }
                for _ in 0..8 {
                    let mut broken = data.clone();
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    broken[state as usize % n] = (state >> 16) as u16;
                    emit(&broken);
                }
            }
        }
    }
    for frame in [Frame::HardReset, Frame::CableReset] {
        let n = tx::encode(frame, Periods::NATIVE_207, &mut reload).unwrap();
        emit(&reload[..n].iter().map(|n| n + 1).collect::<Vec<_>>());
    }
    // Exhaustive truncation and boundary mutations exercise every speculative
    // preamble probe and hoisted payload guard, not just clean packet samples.
    wire.fill(0xa5);
    wire[0] = 0x81;
    wire[1] = 0x60;
    let n = tx::encode(
        Frame::Packet {
            sop: Sop::Sop,
            wire: &wire[..26],
        },
        Periods::NATIVE_207,
        &mut reload,
    )
    .unwrap();
    let data: Vec<_> = reload[..n].iter().map(|n| n + 1).collect();
    for cut in 0..=n {
        emit(&data[..cut]);
    }
    for i in 0..n {
        for value in [0, 14, 15, 25, 26, 29, 30, 50, 51, u16::MAX] {
            let mut changed = data.clone();
            changed[i] = value;
            emit(&changed);
        }
    }
    for suffix in [vec![0, u16::MAX], vec![20; 20], vec![39; 20]] {
        let mut padded = data.clone();
        padded.extend_from_slice(&suffix);
        emit(&padded);
    }
    // Invalid 4b5b words, early EOP, and odd partial bytes must be rejected by
    // byte fusion without reading a second symbol past an EOP or slice end.
    for sop in symbols::WORDS {
        for symbol in 0..32 {
            for prefix in [0x55555u32, 0xaaaaa, 0x34567] {
                let bits = (0..20)
                    .map(|b| prefix >> b & 1)
                    .chain((0..20).map(|b| sop >> b & 1))
                    .chain((0..5).map(|b| symbol >> b & 1))
                    .chain((0..5).map(|b| 13 >> b & 1));
                let mut intervals = Vec::new();
                for bit in bits {
                    if bit == 1 {
                        intervals.extend([20, 20]);
                    } else {
                        intervals.push(39);
                    }
                }
                emit(&intervals);
            }
        }
    }
    for _ in 0..500 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let n = (state % 150) as usize;
        let data: Vec<_> = (0..n)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state % 65) as u16
            })
            .collect();
        emit(&data);
    }
    fs::write(std::env::args().nth(1).expect("output TSV path"), output).unwrap();
}
