//! Read-only factory coefficients from the CRC-protected KM003C settings-A
//! block. Offsets and the correction algorithm follow stock 2.0.7, not a fit to
//! sequential samples taken under a changing load. Never writes calibration.
use crate::pd::crc32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    Checksum,
    LegacyFormat,
    Uncalibrated,
    Coefficient,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calibration {
    voltage_offsets: [i32; 5],
    current_gains: [i32; 10],
    current_offset: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measurement {
    pub voltage_microvolts: u32,
    pub current_microamps: i32,
}
impl Calibration {
    pub fn from_settings(block: &[u8]) -> Result<Self, Error> {
        if block.len() != 96 {
            return Err(Error::Length);
        }
        let word = |offset| i32::from_le_bytes(block[offset..offset + 4].try_into().unwrap());
        if crc32(&block[..92]) != word(92) as u32 {
            return Err(Error::Checksum);
        }
        // Native startup rejects the old representation and may migrate it.
        // A read-only driver must not reinterpret/migrate that format silently.
        if word(0) & 2 != 0 {
            return Err(Error::LegacyFormat);
        }
        if word(0) & 4 != 0 {
            return Err(Error::Uncalibrated);
        }
        let calibration = Self {
            voltage_offsets: core::array::from_fn(|i| word(0x1c + 4 * i)),
            current_gains: core::array::from_fn(|i| word(0x30 + 4 * i)),
            current_offset: word(0x58),
        };
        // Conservative native startup/migration bounds, not invented values
        // to make corrupted flash look like a successfully calibrated device.
        if calibration
            .voltage_offsets
            .iter()
            .any(|v| !(-10000..=10000).contains(v))
            || calibration
                .current_gains
                .iter()
                .any(|g| !(900001..=1099999).contains(g))
            || !(-499..=499).contains(&calibration.current_offset)
        {
            return Err(Error::Coefficient);
        }
        Ok(calibration)
    }
    pub fn correct(self, voltage_microvolts: u32, current_microamps: i32) -> Option<Measurement> {
        let offsets = self.voltage_offsets;
        let offset = if offsets[0] == offsets[1..].iter().sum() {
            offsets[0]
        } else {
            // These thresholds, including the special low-voltage -100 µV
            // offset, reproduce native 0x17b1c exactly.
            match voltage_microvolts {
                0..1500000 => -100,
                1500000..15000000 => offsets[0],
                15000000..25000000 => offsets[1],
                25000000..35000000 => offsets[2],
                35000000..45000000 => offsets[3],
                _ => offsets[4],
            }
        };
        let magnitude = current_microamps.unsigned_abs();
        let range = [2100000, 4100000, 6100000, 8200000]
            .iter()
            .filter(|&&t| magnitude >= t)
            .count();
        let gain = self.current_gains[range + if current_microamps > 0 { 0 } else { 5 }];
        // i64 multiply and signed truncation match the ARM software division.
        let current = (i64::from(current_microamps) - i64::from(self.current_offset))
            * i64::from(gain)
            / 1000000;
        let voltage = (i64::from(voltage_microvolts) + i64::from(offset)).max(0);
        Some(Measurement {
            voltage_microvolts: voltage.try_into().ok()?,
            current_microamps: current.try_into().ok()?,
        })
    }
}

#[cfg(test)]
mod tests;
