//! Two-stage completed-burst decoder. The small scanner returns 5-bit symbols;
//! byte/header/EOP/CRC validation is shared by portable and assembly scanners.
use super::{DATA_SYMBOLS, Error, Event, ORDERED, Packet, Sop, crc32};

pub const CAPACITY: usize = 69;
pub const NO_SOP: u32 = 0x4000_0000;
pub const TIMING: u32 = 0x8000_0000;
pub const TRUNCATED: u32 = 0x8000_0001;
pub const FULL: u32 = 0x8000_0002;
pub const WORDS: [u32; 7] = {
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
pub const NIBBLES: [u8; 32] = {
    let mut out = [255; 32];
    let mut i = 0;
    while i < 16 {
        out[DATA_SYMBOLS[i] as usize] = i as u8;
        i += 1;
    }
    out
};
/// Four-interval BMC batches. Input key is the DSP byte order a,c,b,d;
/// output packs consumed bytes, bit count and LSB-first decoded bits.
pub const BATCH: [u16; 16] = {
    let mut out = [0; 16];
    let mut k = 0;
    while k < 16 {
        let mask = (k & 9) | ((k & 2) << 1) | ((k & 4) >> 1);
        let (mut i, mut bits, mut n, mut bad) = (0, 0, 0, false);
        while i < 4 {
            if mask & (1 << i) == 0 {
                i += 1;
                n += 1;
            } else {
                if i == 3 {
                    break;
                }
                if mask & (1 << (i + 1)) == 0 {
                    bad = true;
                    break;
                }
                bits |= 1 << n;
                n += 1;
                i += 2;
            }
        }
        if !bad {
            out[k] = ((i * 2) << 8) | (n << 4) | bits;
        }
        k += 1;
    }
    out
};

/// Scanner ABI shared with the board's Thumb-2 kernels: bit31=error, bit30=no
/// ordered set, bits10:8=SOP/reset selector, low byte=number of output symbols.
/// MODE 0 reads scalar intervals; MODE 1 tests packed pairs first. `short_pair`
/// is injected so the exact same scanner can compare scalar Rust and DSP.
#[inline(always)]
pub fn scan<const MODE: u8, const FILTER: bool>(
    data: &[u16],
    out: &mut [u8; CAPACITY],
    short_pair: impl Fn(u32) -> bool,
) -> u32 {
    let mut pos = 0usize;
    let mut pending = false;
    let mut next = || -> Result<Option<u8>, u32> {
        if MODE != 0 {
            if pos + 1 < data.len() {
                // Pack halfwords explicitly: no alignment or host-endian assumption.
                let pair = u32::from(data[pos]) | (u32::from(data[pos + 1]) << 16);
                if short_pair(pair) {
                    pos += 2;
                    return Ok(Some(1));
                }
            }
            let Some(&n) = data.get(pos) else {
                return Ok(None);
            };
            pos += 1;
            if n.wrapping_sub(30) <= 20 {
                return Ok(Some(0));
            }
            if n.wrapping_sub(15) <= 10 {
                if pos == data.len() {
                    return Ok(None);
                }
                pos += 1; // A short followed by a non-short is a damaged bit.
            }
            return Err(TIMING);
        }
        while let Some(&n) = data.get(pos) {
            pos += 1;
            if n.wrapping_sub(15) <= 10 {
                pending = !pending;
                if !pending {
                    return Ok(Some(1));
                }
            } else if n.wrapping_sub(30) <= 20 && !pending {
                return Ok(Some(0));
            } else {
                pending = false;
                return Err(TIMING);
            }
        }
        Ok(None)
    };
    let (mut word, mut left) = (0u32, 20u8);
    let sop = loop {
        let b = match next() {
            Ok(Some(b)) => b,
            Ok(None) => return NO_SOP,
            Err(_) => {
                word = 0;
                left = 20;
                continue;
            }
        };
        word = (word >> 1) | (u32::from(b) << 19);
        left = left.saturating_sub(1);
        if left != 0 {
            continue;
        }
        // A preamble alternates 01010/10101; none can finish an ordered set.
        // Gate the expensive seven-word search by the final 5-bit symbol.
        if FILTER && ((0x0202_0040u32 >> (word >> 15)) & 1) == 0 {
            continue;
        }
        if let Some(i) = WORDS.iter().position(|&w| w == word) {
            break i as u32;
        }
    };
    if sop >= 5 {
        return sop << 8;
    }
    let (mut symbol, mut count) = (16u8, 0usize);
    loop {
        let b = match next() {
            Ok(Some(b)) => b,
            Ok(None) => return TRUNCATED,
            Err(e) => return e,
        };
        // The sentinel falls out after five bits, replacing a separate counter.
        let complete = symbol & 1 != 0;
        symbol = (symbol >> 1) | (b << 4);
        if !complete {
            continue;
        }
        out[count] = symbol;
        count += 1;
        if symbol == 13 {
            return (sop << 8) | count as u32;
        }
        if count == CAPACITY {
            return FULL;
        }
        symbol = 16;
    }
}

#[inline(always)]
pub fn scalar_pair(pair: u32) -> bool {
    (pair as u16).wrapping_sub(15) <= 10 && ((pair >> 16) as u16).wrapping_sub(15) <= 10
}

/// Validate scanner output, including malformed status/symbols. Assembly never
/// supplies trusted bytes directly to usbpd: CRC and NDO checks remain here.
#[inline(always)]
pub fn validate(status: u32, symbols: &[u8; CAPACITY]) -> Result<Option<Event>, Error> {
    if status & 0x8000_0000 != 0 {
        return Err(match status {
            TIMING => Error::Timing,
            FULL => Error::MissingEop,
            _ => Error::Truncated,
        });
    }
    if status == NO_SOP {
        return Ok(None);
    }
    let sop = (status >> 8) as usize;
    if sop == 5 {
        return Ok(Some(Event::HardReset));
    }
    if sop == 6 {
        return Ok(Some(Event::CableReset));
    }
    if sop > 4 {
        return Err(Error::Timing);
    }
    let count = (status & 255) as usize;
    if count == 0 || count > CAPACITY || symbols[count - 1] != 13 {
        return Err(Error::MissingEop);
    }
    if count % 2 != 1 || count < 13 {
        return Err(Error::UnexpectedEop);
    }
    let size = (count - 1) / 2;
    let mut bytes = [0u8; 34];
    for (i, dst) in bytes[..size].iter_mut().enumerate() {
        let lo = symbols[2 * i];
        let hi = symbols[2 * i + 1];
        let l = NIBBLES.get(usize::from(lo)).copied().unwrap_or(255);
        let h = NIBBLES.get(usize::from(hi)).copied().unwrap_or(255);
        if l == 255 {
            return Err(Error::InvalidSymbol(lo));
        }
        if h == 255 {
            return Err(Error::InvalidSymbol(hi));
        }
        *dst = l | (h << 4);
    }
    if size != 6 + 4 * usize::from((bytes[1] >> 4) & 7) {
        return Err(Error::UnexpectedEop);
    }
    let len = size - 4;
    let crc = u32::from_le_bytes(bytes[len..size].try_into().unwrap());
    if crc32(&bytes[..len]) != crc {
        return Err(Error::Crc);
    }
    let mut packet = Packet {
        sop: [
            Sop::Sop,
            Sop::Prime,
            Sop::DoublePrime,
            Sop::PrimeDebug,
            Sop::DoublePrimeDebug,
        ][sop],
        bytes: [0; 30],
        len: len as u8,
        crc,
    };
    packet.bytes[..len].copy_from_slice(&bytes[..len]);
    Ok(Some(Event::Packet(packet)))
}
/// Fused scanner output is bytes (including CRC) followed by an EOP marker.
/// Status still counts original 5-bit symbols, so header/NDO bounds and odd
/// partial bytes receive the same checks as the symbol-based decoder.
#[inline(always)]
pub fn validate_bytes(status: u32, bytes: &[u8; 35]) -> Result<Option<Event>, Error> {
    validate_bytes_observed(status, bytes, |_| {})
}
/// Same validation with CRC/assembly boundaries for board-owned DWT profiling.
#[inline(always)]
pub fn validate_bytes_observed(
    status: u32,
    bytes: &[u8; 35],
    mut mark: impl FnMut(super::burst::Phase),
) -> Result<Option<Event>, Error> {
    if status & 0x8000_0000 != 0 {
        return Err(match status {
            TIMING => Error::Timing,
            TRUNCATED => Error::Truncated,
            FULL => Error::MissingEop,
            _ if status & 0xffff_ff00 == 0x8001_0000 => Error::InvalidSymbol(status as u8),
            _ => Error::Timing,
        });
    }
    if status == NO_SOP {
        return Ok(None);
    }
    let sop = (status >> 8) as usize;
    if sop == 5 {
        return Ok(Some(Event::HardReset));
    }
    if sop == 6 {
        return Ok(Some(Event::CableReset));
    }
    if sop > 4 {
        return Err(Error::Timing);
    }
    let count = (status & 255) as usize;
    if !(13..=69).contains(&count) || count % 2 != 1 {
        return Err(Error::UnexpectedEop);
    }
    let size = (count - 1) / 2;
    if bytes[size] != 13 {
        return Err(Error::MissingEop);
    }
    if size != 6 + 4 * usize::from((bytes[1] >> 4) & 7) {
        return Err(Error::UnexpectedEop);
    }
    let len = size - 4;
    let crc = u32::from_le_bytes(bytes[len..size].try_into().unwrap());
    mark(super::burst::Phase::Crc);
    if crc32(&bytes[..len]) != crc {
        return Err(Error::Crc);
    }
    mark(super::burst::Phase::Assemble);
    let mut packet = Packet {
        sop: [
            Sop::Sop,
            Sop::Prime,
            Sop::DoublePrime,
            Sop::PrimeDebug,
            Sop::DoublePrimeDebug,
        ][sop],
        bytes: [0; 30],
        len: len as u8,
        crc,
    };
    packet.bytes[..len].copy_from_slice(&bytes[..len]);
    Ok(Some(Event::Packet(packet)))
}

#[inline(always)]
pub fn decode<const MODE: u8, const FILTER: bool>(
    data: &[u16],
    pair: impl Fn(u32) -> bool,
) -> Result<Option<Event>, Error> {
    let mut output = [0; CAPACITY];
    let status = scan::<MODE, FILTER>(data, &mut output, pair);
    validate(status, &output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fused_bytes_require_eop_shape_and_crc() {
        let mut bytes = [0; 35];
        bytes[..2].copy_from_slice(&[0x81, 0]);
        bytes[2..6].copy_from_slice(&crc32(&[0x81, 0]).to_le_bytes());
        bytes[6] = 13;
        assert!(matches!(
            validate_bytes(13, &bytes),
            Ok(Some(Event::Packet(_)))
        ));
        let mut bad = bytes;
        bad[0] ^= 1;
        assert_eq!(validate_bytes(13, &bad), Err(Error::Crc));
        bad = bytes;
        bad[1] = 0x10;
        assert_eq!(validate_bytes(13, &bad), Err(Error::UnexpectedEop));
        bad = bytes;
        bad[6] = 0;
        assert_eq!(validate_bytes(13, &bad), Err(Error::MissingEop));
        for count in [0, 1, 12, 14, 68, 70, 255] {
            assert_eq!(validate_bytes(count, &bytes), Err(Error::UnexpectedEop));
        }
        for status in [TIMING, TRUNCATED, FULL, 0x8000_0003] {
            assert!(validate_bytes(status, &bytes).is_err());
        }
        assert_eq!(validate_bytes(TRUNCATED, &bytes), Err(Error::Truncated));
        assert_eq!(validate_bytes(FULL, &bytes), Err(Error::MissingEop));
        assert_eq!(
            validate_bytes(0x8001_0007, &bytes),
            Err(Error::InvalidSymbol(7))
        );
        assert_eq!(validate_bytes(NO_SOP, &bytes), Ok(None));
        assert_eq!(validate_bytes(5 << 8, &bytes), Ok(Some(Event::HardReset)));
        assert_eq!(validate_bytes(6 << 8, &bytes), Ok(Some(Event::CableReset)));
    }
}
