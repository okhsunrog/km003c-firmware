//! Decode a text export of edge intervals without any device connection.
use km003c_drivers::pd::{Decoder, Event};
use std::{env, fs, io};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err(invalid("usage: pd_decode FILE HALF_BIT_TICKS TOLERANCE_TICKS").into());
    }
    let mut decoder = Decoder::new(args[2].parse()?, args[3].parse()?)
        .ok_or_else(|| invalid("invalid timebase/tolerance"))?;
    let mut intervals = 0;
    let mut packets = 0;
    let mut errors = 0;
    for line in fs::read_to_string(&args[1])?.lines() {
        // Plain u16 deltas, decimal or 0x-prefixed, whitespace/comma separated.
        // No timestamp-to-delta conversion is guessed by this tool.
        for token in line
            .split('#')
            .next()
            .unwrap()
            .split(|c: char| c.is_whitespace() || c == ',')
        {
            if token.is_empty() {
                continue;
            }
            let ticks = if let Some(hex) = token.strip_prefix("0x") {
                u16::from_str_radix(hex, 16)?
            } else {
                token.parse()?
            };
            intervals += 1;
            match decoder.push_interval(ticks) {
                Ok(Some(Event::Packet(packet))) => {
                    packets += 1;
                    println!(
                        "{intervals}: {:?} header={:04x} wire={:02x?} crc={:08x}",
                        packet.sop,
                        packet.header(),
                        packet.wire(),
                        packet.crc
                    );
                    // usbpd's Driver contract carries SOP header/data bytes
                    // only. Do not feed cable-addressed SOP' packets into its
                    // port policy parser as if they came from the partner.
                    #[cfg(feature = "usbpd")]
                    if packet.sop == km003c_drivers::pd::Sop::Sop {
                        println!(
                            "  usbpd: {:?}",
                            usbpd::protocol_layer::message::Message::from_bytes(packet.wire())
                        );
                    }
                }
                Ok(Some(event)) => println!("{intervals}: {event:?}"),
                Ok(None) => (),
                Err(error) => {
                    errors += 1;
                    eprintln!("{intervals}: {error:?}");
                }
            }
        }
    }
    if let Err(error) = decoder.finish() {
        errors += 1;
        eprintln!("end: {error:?}");
    }
    eprintln!("{intervals} intervals; {packets} CRC-valid packets; {errors} errors");
    Ok(())
}
