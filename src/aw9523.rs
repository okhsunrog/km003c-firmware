//! AW9523B at 0x5b on PC13/PH2 I2C3. Preserve unrelated expander outputs.
//! P1_1 is panel RESET; native dimming registers 0x22/0x23 drive backlight.
use embassy_hc32::{
    gpio::{AlternateMode, AlternatePin, Port},
    i2c::{self, Error, I2c3},
    intc::{Line, Route},
    pac::interrupt,
};
use embedded_hal_async::i2c::I2c;
#[interrupt]
fn INT110() {
    i2c::on_interrupt();
}
#[interrupt]
fn INT111() {
    i2c::on_interrupt();
}
#[interrupt]
fn INT112() {
    i2c::on_interrupt();
}
#[interrupt]
fn INT113() {
    i2c::on_interrupt();
}

type BusMutex = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
pub type Device = embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice<'static, BusMutex, I2c3>;
type DeviceError = embassy_embedded_hal::shared_bus::I2cDeviceError<Error>;
pub type SharedBus =
    embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, I2c3>;
#[derive(Clone, Copy)]
pub struct Aw9523 {
    #[cfg(feature = "orientation")]
    bus: &'static SharedBus,
    registers: &'static embassy_sync::mutex::Mutex<BusMutex, Device>,
}

impl Aw9523 {
    /// Board-exclusive I2C3 and PC13/PH2 ownership; clocks must be configured.
    pub unsafe fn new() -> (Self, [Route; 4]) {
        let bus = unsafe {
            I2c3::new(
                48_000_000,
                100_000,
                [
                    AlternatePin::new(Port::C, 13, 48, AlternateMode::FunctionOnly),
                    AlternatePin::new(Port::H, 2, 49, AlternateMode::FunctionOnly),
                ],
            )
            .unwrap()
        };
        let routes = unsafe {
            [
                Route::new(428, Line::new(110).unwrap(), 8).unwrap(),
                Route::new(429, Line::new(111).unwrap(), 8).unwrap(),
                Route::new(430, Line::new(112).unwrap(), 8).unwrap(),
                Route::new(431, Line::new(113).unwrap(), 8).unwrap(),
            ]
        };
        static BUS: static_cell::StaticCell<SharedBus> = static_cell::StaticCell::new();
        let bus = BUS.init(SharedBus::new(bus));
        static DEVICE: static_cell::StaticCell<embassy_sync::mutex::Mutex<BusMutex, Device>> =
            static_cell::StaticCell::new();
        let registers = DEVICE.init(embassy_sync::mutex::Mutex::new(Device::new(bus)));
        (
            Self {
                #[cfg(feature = "orientation")]
                bus,
                registers,
            },
            routes,
        )
    }
    #[cfg(feature = "orientation")]
    pub fn sensor_bus(&self) -> Device {
        Device::new(self.bus)
    }
    async fn read_reg(bus: &mut Device, register: u8) -> Result<u8, DeviceError> {
        let mut byte = [0];
        bus.write_read(0x5b, &[register], &mut byte).await?;
        Ok(byte[0])
    }
    async fn write_reg(bus: &mut Device, register: u8, value: u8) -> Result<(), DeviceError> {
        bus.write(0x5b, &[register, value]).await
    }
    // Set output latch before enabling RESET's output driver. This avoids an
    // unintended low pulse while converting the pin to GPIO/output mode.
    pub async fn lcd_reset(&self) -> Result<(), DeviceError> {
        // Keep the lock across the reset sequence: the final output write
        // uses the snapshot from before the delay, including unrelated pins.
        let mut bus = self.registers.lock().await;
        let id = Self::read_reg(&mut bus, 0x10).await?;
        log::info!("AW9523B ID={id:02x}");
        assert_eq!(id, 0x23, "Unexpected GPIO expander ID");
        // RMW mode/direction/output: PD/CC and other lines share this expander.
        let mode = Self::read_reg(&mut bus, 0x13).await?;
        Self::write_reg(&mut bus, 0x13, (mode | 2) & !0x0c).await?; // RESET GPIO; native backlight LED channels.
        let output = Self::read_reg(&mut bus, 3).await?;
        Self::write_reg(&mut bus, 3, output | 2).await?;
        let direction = Self::read_reg(&mut bus, 5).await?;
        Self::write_reg(&mut bus, 5, direction & !2).await?;
        Self::write_reg(&mut bus, 3, output & !2).await?;
        embassy_time::Timer::after_millis(50).await;
        Self::write_reg(&mut bus, 3, output | 2).await?;
        embassy_time::Timer::after_millis(10).await;
        Ok(())
    }
    pub async fn backlight(&self, level: u8) -> Result<(), DeviceError> {
        let mut bus = self.registers.lock().await;
        // Same two dimming registers as native boot routine 0x11be.
        Self::write_reg(&mut bus, 0x22, level).await?;
        Self::write_reg(&mut bus, 0x23, level / 2).await
    }
    #[cfg(feature = "pd-tx-test")]
    pub async fn cc2_rd(&self, enabled: bool) -> Result<(), crate::cc_frontend::Error> {
        use crate::cc_frontend::Error as E;
        const OUTPUT0: u8 = 2;
        const CONFIG0: u8 = 4;
        const MODE0: u8 = 0x12;
        const CC2_RD: u8 = 1 << 3;
        let mut bus = self.registers.lock().await;
        let operation = async {
            let id = Self::read_reg(&mut bus, 0x10).await.map_err(|_| E::Bus)?;
            if id != 0x23 {
                return Err(E::Identity);
            }
            let output = Self::read_reg(&mut bus, OUTPUT0)
                .await
                .map_err(|_| E::Bus)?;
            let config = Self::read_reg(&mut bus, CONFIG0)
                .await
                .map_err(|_| E::Bus)?;
            if enabled {
                let mode = Self::read_reg(&mut bus, MODE0).await.map_err(|_| E::Bus)?;
                // Native 2.0.7 0x40b12(2) -> 0x1a5ae(8,2): low latch,
                // then output enable. P0_3 is GPIO; other pins are preserved.
                Self::write_reg(&mut bus, OUTPUT0, output & !CC2_RD)
                    .await
                    .map_err(|_| E::Bus)?;
                Self::write_reg(&mut bus, MODE0, mode | CC2_RD)
                    .await
                    .map_err(|_| E::Bus)?;
                Self::write_reg(&mut bus, CONFIG0, config & !CC2_RD)
                    .await
                    .map_err(|_| E::Bus)?;
            } else {
                // Native release uses input/high-Z, never push-pull high.
                Self::write_reg(&mut bus, CONFIG0, config | CC2_RD)
                    .await
                    .map_err(|_| E::Bus)?;
            }
            let actual = Self::read_reg(&mut bus, CONFIG0)
                .await
                .map_err(|_| E::Bus)?;
            if (actual & CC2_RD == 0) != enabled {
                return Err(E::Readback);
            }
            if enabled
                && Self::read_reg(&mut bus, OUTPUT0)
                    .await
                    .map_err(|_| E::Bus)?
                    & CC2_RD
                    != 0
            {
                return Err(E::Readback);
            }
            Ok(())
        };
        embassy_time::with_timeout(embassy_time::Duration::from_millis(30), operation)
            .await
            .map_err(|_| E::Timeout)?
    }
}
