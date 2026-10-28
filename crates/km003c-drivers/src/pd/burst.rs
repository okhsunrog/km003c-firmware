//! Decode one completed hardware burst. Unlike the streaming API this keeps
//! bit/symbol state in local variables and produces a Packet only once, at EOP.
//! After SOP, damaged data rejects the burst; resynchronization is next burst.
use super::{DATA_SYMBOLS, Decoder, Error, Event, ORDERED, Packet, Sop, crc32};
const WORDS: [u32; 7] = {
    let mut out = [0; 7];
    let mut i = 0;
    while i < 7 {
        let mut j = 0;
        while j < 4 {
            out[i] |= (ORDERED[i][j] as u32) << (5 * j);
            j += 1;
        }
        i += 1;
    }
    out
};
const NIBBLES: [u8; 32] = {
    let mut out = [255; 32];
    let mut i = 0;
    while i < 16 {
        out[DATA_SYMBOLS[i] as usize] = i as u8;
        i += 1;
    }
    out
};
#[inline(always)]
fn bit(
    it: &mut core::slice::Iter<'_, u16>,
    pending: &mut bool,
    half: u16,
    tol: u16,
) -> Result<Option<u8>, Error> {
    for &n in it {
        if n.abs_diff(half) <= tol {
            *pending = !*pending;
            if !*pending {
                return Ok(Some(1));
            }
        } else if n.abs_diff(2 * half) <= 2 * tol && !*pending {
            return Ok(Some(0));
        } else {
            *pending = false;
            return Err(Error::Timing);
        }
    }
    Ok(None)
}
/// Intervals exclude the capture's unknown first-edge duration. CRC is required
/// before delivering any packet; pre-SOP noise can reset the ordered-set search.
#[inline(always)]
pub fn decode(intervals: &[u16], half: u16, tol: u16) -> Result<Option<Event>, Error> {
    decode_observed(intervals, half, tol, |_| {})
}
/// Boundaries for an optional board-owned profiler; no hardware clock is used
/// by the decoder itself. The normal decode entry point eliminates the hook.
#[derive(Clone, Copy)]
pub enum Phase {
    Start,
    Data,
    Crc,
    Assemble,
}
/// Decode with four phase markers, typically sampled with a CPU cycle counter.
#[inline(always)]
pub fn decode_observed(
    intervals: &[u16],
    half: u16,
    tol: u16,
    mark: impl FnMut(Phase),
) -> Result<Option<Event>, Error> {
    decode_configured::<false>(intervals, half, tol, mark)
}
/// A/B benchmark entry: identical decoder with an optional ordered-set filter.
#[inline(always)]
pub fn decode_configured<const FILTER: bool>(
    intervals: &[u16],
    half: u16,
    tol: u16,
    mut mark: impl FnMut(Phase),
) -> Result<Option<Event>, Error> {
    mark(Phase::Start);
    if Decoder::new(half, tol).is_none() {
        return Err(Error::Timing);
    }
    let mut it = intervals.iter();
    let (mut pending, mut word, mut seen) = (false, 0u32, 0u8);
    let sop = loop {
        let b = match bit(&mut it, &mut pending, half, tol) {
            Ok(Some(b)) => b,
            Ok(None) => return Ok(None),
            Err(_) => {
                word = 0;
                seen = 0;
                continue;
            }
        };
        word = (word >> 1) | (u32::from(b) << 19);
        seen = seen.saturating_add(1);
        if seen >= 20
            && (!FILTER || ((0x0202_0040u32 >> (word >> 15)) & 1) != 0)
            && let Some(i) = WORDS.iter().position(|&w| w == word)
        {
            if i == 5 {
                return Ok(Some(Event::HardReset));
            }
            if i == 6 {
                return Ok(Some(Event::CableReset));
            }
            break [
                Sop::Sop,
                Sop::Prime,
                Sop::DoublePrime,
                Sop::PrimeDebug,
                Sop::DoublePrimeDebug,
            ][i];
        }
    };
    let mut bytes = [0u8; 34];
    let (mut used, mut expected) = (0usize, 0usize);
    let (mut symbol, mut bits, mut low, mut high) = (0u8, 0u8, 0u8, false);
    mark(Phase::Data);
    while let Some(b) = bit(&mut it, &mut pending, half, tol)? {
        symbol |= b << bits;
        bits += 1;
        if bits != 5 {
            continue;
        }
        if symbol == 0x0d {
            if expected == 0 || used != expected || high {
                return Err(Error::UnexpectedEop);
            }
            let len = used - 4;
            let crc = u32::from_le_bytes(bytes[len..used].try_into().unwrap());
            mark(Phase::Crc);
            if crc32(&bytes[..len]) != crc {
                return Err(Error::Crc);
            }
            mark(Phase::Assemble);
            let mut packet = Packet {
                sop,
                bytes: [0; 30],
                len: len as u8,
                crc,
            };
            packet.bytes[..len].copy_from_slice(&bytes[..len]);
            return Ok(Some(Event::Packet(packet)));
        }
        if used == bytes.len() || (expected != 0 && used == expected) {
            return Err(Error::MissingEop);
        }
        let nibble = NIBBLES[usize::from(symbol)];
        if nibble == 255 {
            return Err(Error::InvalidSymbol(symbol));
        }
        if high {
            bytes[used] = low | (nibble << 4);
            used += 1;
            if used == 2 {
                expected = 6 + 4 * usize::from((bytes[1] >> 4) & 7);
            }
        } else {
            low = nibble;
        }
        high = !high;
        symbol = 0;
        bits = 0;
    }
    Err(Error::Truncated)
}
