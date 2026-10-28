use super::*;
extern crate std;
use std::{vec, vec::Vec};

// Test encoder only builds synthetic physical intervals; it never calls the
// decoder's bit/symbol helpers. Actual DMA acceptance remains a hardware task.
const ENCODE: [u8; 16] = [
    30, 9, 20, 21, 10, 11, 14, 15, 18, 19, 22, 23, 26, 27, 28, 29,
];
fn symbol(bits: &mut Vec<bool>, symbol: u8) {
    bits.extend((0..5).map(|bit| symbol & (1 << bit) != 0));
}
fn frame(wire: &[u8], ordered: [u8; 4], crc: u32) -> Vec<bool> {
    let mut bits: Vec<bool> = (0..64).map(|n| n % 2 != 0).collect();
    for s in ordered {
        symbol(&mut bits, s);
    }
    for &byte in wire.iter().chain(crc.to_le_bytes().iter()) {
        symbol(&mut bits, ENCODE[usize::from(byte & 15)]);
        symbol(&mut bits, ENCODE[usize::from(byte >> 4)]);
    }
    symbol(&mut bits, 13);
    bits
}
fn feed(decoder: &mut Decoder, bits: &[bool], jitter: bool) -> Vec<Result<Event, Error>> {
    let mut events = Vec::new();
    for (index, &bit) in bits.iter().enumerate() {
        let offset = if jitter { (index % 3) as i32 - 1 } else { 0 };
        for ticks in if bit {
            vec![(20 + offset) as u16; 2]
        } else {
            vec![(40 + offset) as u16]
        } {
            match decoder.push_interval(ticks) {
                Ok(Some(e)) => events.push(Ok(e)),
                Err(e) => events.push(Err(e)),
                Ok(None) => (),
            }
        }
    }
    events
}
#[test]
fn independent_crc_check_vector() {
    assert_eq!(crc32(b"123456789"), 0xcbf43926);
    assert_eq!(crc32(&[]), 0);
}
#[test]
fn known_goodcrc_frame_and_jitter() {
    // Header 0x0001 (GoodCRC, message ID 0), CRC cross-checked with Python zlib.
    let bits = frame(&[1, 0], [24, 24, 24, 17], 0x58c223be);
    let mut d = Decoder::new(20, 3).unwrap();
    let events = feed(&mut d, &bits, true);
    let [Ok(Event::Packet(packet))] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(packet.sop, Sop::Sop);
    assert_eq!(packet.header(), 1);
    assert_eq!(packet.wire(), &[1, 0]);
    assert_eq!(d.finish(), Ok(()));
}
#[test]
fn all_sops_max_payload_and_extended_chunks() {
    for (i, ordered) in [
        [24, 24, 24, 17],
        [24, 24, 6, 6],
        [24, 6, 24, 6],
        [24, 25, 17, 6],
        [24, 25, 6, 17],
    ]
    .into_iter()
    .enumerate()
    {
        let mut wire = [0xa5; 30];
        wire[..2].copy_from_slice(&0xf081u16.to_le_bytes()); // extended + seven objects
        let mut d = Decoder::new(20, 4).unwrap();
        let events = feed(&mut d, &frame(&wire, ordered, crc32(&wire)), false);
        let [Ok(Event::Packet(packet))] = events.as_slice() else {
            panic!("{events:?}");
        };
        assert_eq!(packet.wire(), &wire);
        assert_eq!(
            packet.sop,
            [
                Sop::Sop,
                Sop::Prime,
                Sop::DoublePrime,
                Sop::PrimeDebug,
                Sop::DoublePrimeDebug
            ][i]
        );
    }
}
#[test]
fn rejects_crc_and_recovers_for_next_packet() {
    let mut d = Decoder::new(20, 3).unwrap();
    assert_eq!(
        feed(&mut d, &frame(&[1, 0], [24, 24, 24, 17], 0), false),
        [Err(Error::Crc)]
    );
    let events = feed(&mut d, &frame(&[1, 0], [24, 24, 24, 17], 0x58c223be), false);
    assert!(matches!(events.as_slice(), [Ok(Event::Packet(_))]));
}
#[test]
fn resets_timing_truncation_and_eop_errors() {
    assert!(Decoder::new(0, 0).is_none());
    assert!(Decoder::new(u16::MAX, 0).is_none());
    assert!(Decoder::new(20, 6).is_none());
    let mut d = Decoder::new(20, 3).unwrap();
    assert_eq!(d.push_interval(0), Err(Error::Timing));
    assert_eq!(d.push_interval(20), Ok(None));
    assert_eq!(d.push_interval(40), Err(Error::Timing));
    let bits = frame(&[1, 0], [24, 24, 24, 17], 0x58c223be);
    assert!(feed(&mut d, &bits[..bits.len() - 5], false).is_empty());
    assert_eq!(d.finish(), Err(Error::Truncated));
    let mut early_eop = bits[..84].to_vec(); // preamble + SOP
    symbol(&mut early_eop, 13);
    assert_eq!(feed(&mut d, &early_eop, false), [Err(Error::UnexpectedEop)]);
    let mut invalid = bits[..84].to_vec();
    symbol(&mut invalid, 0);
    assert_eq!(
        feed(&mut d, &invalid, false),
        [Err(Error::InvalidSymbol(0))]
    );
    let mut missing = bits[..bits.len() - 5].to_vec();
    symbol(&mut missing, 30);
    assert_eq!(feed(&mut d, &missing, false), [Err(Error::MissingEop)]);
}
#[test]
fn reset_ordered_sets_and_back_to_back_frames() {
    let mut d = Decoder::new(20, 3).unwrap();
    for (set, event) in [
        ([7, 7, 7, 25], Event::HardReset),
        ([7, 24, 7, 6], Event::CableReset),
    ] {
        let mut bits = Vec::new();
        for s in set {
            symbol(&mut bits, s);
        }
        assert_eq!(feed(&mut d, &bits, false), [Ok(event)]);
    }
    let bits = frame(&[1, 0], [24, 24, 24, 17], 0x58c223be);
    for _ in 0..2 {
        assert!(matches!(
            feed(&mut d, &bits, false).as_slice(),
            [Ok(Event::Packet(_))]
        ));
    }
}

#[test]
fn noise_and_edge_loss_do_not_poison_later_packets() {
    let mut d = Decoder::new(20, 3).unwrap();
    let mut state = 0x12345678u32;
    for _ in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let _ = d.push_interval(state as u16);
    }
    let _ = d.finish();
    let mut broken = frame(&[1, 0], [24, 24, 24, 17], 0x58c223be);
    broken.remove(100);
    let results = feed(&mut d, &broken, false);
    assert!(!results.iter().any(|v| matches!(v, Ok(Event::Packet(_)))));
    let _ = d.finish();
    assert!(matches!(
        feed(&mut d, &frame(&[1, 0], [24, 24, 24, 17], 0x58c223be), true).as_slice(),
        [Ok(Event::Packet(_))]
    ));
}

#[test]
fn measured_pixel8pro_pps_request_with_filter_asymmetry() {
    // Real HW2.1 DMA capture, not generated by the synthetic encoder above.
    // First edge starts the timer; no preceding interval exists for that edge.
    let input = include_str!("../../tests/data/pixel8pro-request.intervals");
    let ticks: Vec<u16> = input
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(ticks.len(), 292);
    let mut decoder = Decoder::new(20, 5).unwrap();
    let mut packets = Vec::new();
    for &tick in &ticks[1..] {
        if let Some(Event::Packet(packet)) = decoder.push_interval(tick).unwrap() {
            packets.push(packet);
        }
    }
    decoder.finish().unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].sop, Sop::Sop);
    assert_eq!(packets[0].wire(), &[0x82, 0x16, 0x26, 0x9a, 0x03, 0x63]);
    assert_eq!(packets[0].crc, 0x8751d8b7);
}
