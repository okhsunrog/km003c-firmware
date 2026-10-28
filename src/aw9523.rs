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

pub struct Aw9523 {
    bus: I2c3,
    _routes: [Route; 4],
}
impl Aw9523 {
    /// Board-exclusive I2C3 and PC13/PH2 ownership; clocks must be configured.
    pub unsafe fn new() -> Self {
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
        Self {
            bus,
            _routes: routes,
        }
    }
    pub async fn read(&mut self, register: u8) -> Result<u8, Error> {
        let mut byte = [0];
        self.bus.write_read(0x5b, &[register], &mut byte).await?;
        Ok(byte[0])
    }
    async fn write(&mut self, register: u8, value: u8) -> Result<(), Error> {
        self.bus.write(0x5b, &[register, value]).await
    }
    // Set output latch before enabling RESET's output driver. This avoids an
    // unintended low pulse while converting the pin to GPIO/output mode.
    pub async fn lcd_reset(&mut self) -> Result<(), Error> {
        let id = self.read(0x10).await?;
        log::info!("AW9523B ID={id:02x}");
        assert_eq!(id, 0x23, "Unexpected GPIO expander ID");
        // RMW mode/direction/output: PD/CC and other lines share this expander.
        let mode = self.read(0x13).await?;
        self.write(0x13, (mode | 2) & !0x0c).await?; // RESET GPIO; native backlight LED channels.
        let output = self.read(3).await?;
        self.write(3, output | 2).await?;
        let direction = self.read(5).await?;
        self.write(5, direction & !2).await?;
        self.write(3, output & !2).await?;
        embassy_time::Timer::after_millis(50).await;
        self.write(3, output | 2).await?;
        embassy_time::Timer::after_millis(10).await;
        Ok(())
    }
    pub async fn backlight(&mut self, level: u8) -> Result<(), Error> {
        // Same two dimming registers as native boot routine 0x11be.
        self.write(0x22, level).await?;
        self.write(0x23, level / 2).await
    }
}
