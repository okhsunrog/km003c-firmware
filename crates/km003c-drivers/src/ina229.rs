//! INA229 SPI register protocol (TI SLYS023A §§7.5–7.6).
//!
//! Recovered KM003C SPI2 code matches this protocol. Probe the actual board ID
//! before relying on this candidate driver or writing configuration.
use embedded_hal::spi::SpiDevice;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub manufacturer: u16,
    pub device: u16,
}
impl Identity {
    pub fn is_ina229(self) -> bool {
        self.manufacturer == 0x5449 && self.device >> 4 == 0x229
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    UnexpectedIdentity(Identity),
    InvalidConfiguration,
}

/// Explicit register values: there is no implicit reset or guessed shunt value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub config: u16,
    pub adc_config: u16,
    pub shunt_cal: u16,
    pub shunt_tempco: u16,
}
impl Configuration {
    /// Recovered from native init 0x6f9a in both preserved 1.9.9 images.
    /// This calibration value alone does not establish CURRENT_LSB.
    pub const NATIVE_199: Self = Self {
        config: 0x0170,
        adc_config: 0xfdb0,
        shunt_cal: 0x1062,
        shunt_tempco: 0x001e,
    };
    pub fn valid(self) -> bool {
        // Exclude reset/accumulator reset and reserved bits. SHUNT_CAL=0
        // cannot provide useful current results. No undocumented writes.
        self.config & 0xc00f == 0
            && self.shunt_cal != 0
            && self.shunt_cal & 0x8000 == 0
            && self.shunt_tempco & 0xc000 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub config: u16,
    pub shunt_cal: u16,
    pub bus_counts: u32,
    pub shunt_counts: i32,
    pub current_counts: i32,
    pub temperature_counts: i16,
    /// Reading DIAG_ALRT clears some latched flags, including conversion-ready.
    pub diagnostic: u16,
}
impl Sample {
    /// 195.3125 µV/count, expressed in nanovolts without floating point.
    pub fn bus_nanovolts(self) -> u64 {
        u64::from(self.bus_counts) * 390625 / 2
    }
    /// 312.5 or 78.125 nV/count according to the *read* CONFIG.ADCRANGE bit.
    pub fn shunt_picovolts(self) -> i64 {
        i64::from(self.shunt_counts)
            * if self.config & 0x10 == 0 {
                312500
            } else {
                78125
            }
    }
    pub fn temperature_millicelsius(self) -> i32 {
        i32::from(self.temperature_counts) * 125 / 16
    }
    pub fn math_overflow(self) -> bool {
        self.diagnostic & (1 << 9) != 0
    }
    // No amperes/power conversion until CURRENT_LSB and board calibration are known.
}

pub struct Ina229<S> {
    spi: S,
}
impl<S: SpiDevice<u8>> Ina229<S> {
    /// Construction itself performs no I/O or writes to the sensor.
    pub fn new(spi: S) -> Self {
        Self { spi }
    }
    pub fn release(self) -> S {
        self.spi
    }
    fn read<const N: usize>(&mut self, register: u8) -> Result<[u8; N], Error<S::Error>> {
        let mut data = [0; N];
        // One transaction holds CS low across address and MSB-first response.
        self.spi
            .transaction(&mut [
                embedded_hal::spi::Operation::Write(&[(register << 2) | 1]),
                embedded_hal::spi::Operation::Read(&mut data),
            ])
            .map_err(Error::Bus)?;
        Ok(data)
    }
    fn read16(&mut self, register: u8) -> Result<u16, Error<S::Error>> {
        Ok(u16::from_be_bytes(self.read(register)?))
    }
    fn read24(&mut self, register: u8) -> Result<u32, Error<S::Error>> {
        let [a, b, c] = self.read(register)?;
        Ok(u32::from_be_bytes([0, a, b, c]))
    }
    fn write16(&mut self, register: u8, value: u16) -> Result<(), Error<S::Error>> {
        let [a, b] = value.to_be_bytes();
        self.spi.write(&[register << 2, a, b]).map_err(Error::Bus)
    }
    pub fn identity(&mut self) -> Result<Identity, Error<S::Error>> {
        Ok(Identity {
            manufacturer: self.read16(0x3e)?,
            device: self.read16(0x3f)?,
        })
    }
    pub fn configuration(&mut self) -> Result<Configuration, Error<S::Error>> {
        Ok(Configuration {
            config: self.read16(0)?,
            adc_config: self.read16(1)?,
            shunt_cal: self.read16(2)?,
            shunt_tempco: self.read16(3)?,
        })
    }
    /// A mismatched/floating SPI bus must never receive configuration writes.
    /// Writes are sequential, not atomic; on error re-probe and inspect state.
    pub fn configure(&mut self, config: Configuration) -> Result<(), Error<S::Error>> {
        if !config.valid() {
            return Err(Error::InvalidConfiguration);
        }
        let id = self.identity()?;
        if !id.is_ina229() {
            return Err(Error::UnexpectedIdentity(id));
        }
        self.write16(0, config.config)?;
        self.write16(2, config.shunt_cal)?;
        self.write16(3, config.shunt_tempco)?;
        // Conversion mode last, after range and calibration.
        self.write16(1, config.adc_config)
    }
    /// Separate reads are not an atomic ADC snapshot. A conversion can finish
    /// between transactions in continuous mode; flag freshness is advisory.
    pub fn sample(&mut self) -> Result<Sample, Error<S::Error>> {
        Ok(Sample {
            config: self.read16(0)?,
            shunt_cal: self.read16(2)?,
            bus_counts: self.read24(5)? >> 4,
            shunt_counts: signed20(self.read24(4)?),
            current_counts: signed20(self.read24(7)?),
            temperature_counts: self.read16(6)? as i16,
            diagnostic: self.read16(0x0b)?,
        })
    }
}

/// Left-justified signed 20-bit result in the low 24 bits of a register.
fn signed20(raw: u32) -> i32 {
    ((raw << 8) as i32) >> 12
}

#[cfg(test)]
mod tests;
