//! Silan SC7A20 family, WHO_AM_I=0x11. No calibration/NVM/self-test writes.
//! Bus is borrowed per operation, allowing one board task to share it with GPIO.
use embedded_hal_async::i2c::I2c;
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    Identity(u8),
    Configuration,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub xyz_mg: [i16; 3],
    pub source: u8,
    pub status: u8,
}
pub struct Sc7a20;
impl Sc7a20 {
    async fn read<B: I2c>(bus: &mut B, reg: u8) -> Result<u8, Error<B::Error>> {
        let mut b = [0];
        bus.write_read(0x19, &[reg], &mut b)
            .await
            .map_err(Error::Bus)?;
        Ok(b[0])
    }
    pub async fn configure<B: I2c>(bus: &mut B) -> Result<Self, Error<B::Error>> {
        let id = Self::read(bus, 0x0f).await?;
        if id != 0x11 {
            return Err(Error::Identity(id));
        }
        // Stop acquisition while configuring. HR +/-2g, BDU prevents tearing
        // within axis words; interrupt pins remain unrouted (CTRL3/6=0).
        for (reg, value) in [
            (0x20, 0x07),
            (0x21, 0xc0),
            (0x22, 0),
            (0x23, 0x88),
            (0x24, 0),
            (0x25, 0),
            (0x30, 0xff),
            (0x32, 0x0f),
            (0x20, 0x27),
        ] {
            bus.write(0x19, &[reg, value]).await.map_err(Error::Bus)?;
        }
        for (reg, value) in [
            (0x20, 0x27),
            (0x21, 0xc0),
            (0x23, 0x88),
            (0x30, 0xff),
            (0x32, 0x0f),
        ] {
            if Self::read(bus, reg).await? != value {
                return Err(Error::Configuration);
            }
        }
        Ok(Self)
    }
    pub async fn sample<B: I2c>(&mut self, bus: &mut B) -> Result<Sample, Error<B::Error>> {
        let mut bytes = [0; 7];
        // I2C subaddress bit 7 enables auto-increment: STATUS followed by XYZ.
        bus.write_read(0x19, &[0xa7], &mut bytes)
            .await
            .map_err(Error::Bus)?;
        let source = Self::read(bus, 0x31).await?;
        Ok(Sample {
            status: bytes[0],
            source,
            xyz_mg: core::array::from_fn(|i| {
                i16::from_le_bytes([bytes[1 + 2 * i], bytes[2 + 2 * i]]) >> 4
            }),
        })
    }
}
/// The native 1.9.9 table maps XL/XH/YL/YH to these four quarter turns.
/// Flat, ambiguous or unasserted orientations preserve the previous direction.
pub fn direction(source: u8) -> Option<u8> {
    if source & 0x40 == 0 {
        return None;
    }
    match source & 0x0f {
        1 => Some(0),
        2 => Some(2),
        4 => Some(1),
        8 => Some(3),
        _ => None,
    }
}
#[derive(Default)]
pub struct Filter {
    direction: u8,
    candidate: Option<u8>,
    repeats: u8,
}
impl Filter {
    /// Require three fresh 10 Hz observations (~300 ms), without allowing an
    /// ambiguous/flat sample to complete an earlier partial transition.
    pub fn update(&mut self, sample: Sample) -> u8 {
        if sample.status & 8 == 0 {
            return self.direction;
        }
        let next = direction(sample.source);
        if next != self.candidate {
            self.candidate = next;
            self.repeats = 0;
        }
        self.repeats = self.repeats.saturating_add(1);
        if self.repeats >= 3
            && let Some(next) = next
        {
            self.direction = next;
        }
        self.direction
    }
}
#[cfg(test)]
mod tests;
