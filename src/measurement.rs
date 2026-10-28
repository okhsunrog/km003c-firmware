//! Optional SPI2 measurement task. Configure the identified INA229 explicitly;
//! read factory coefficients without changing the native settings storage.
//! Enable explicitly with `--features measurement` after reviewing board pins.
use core::cell::Cell;
use embassy_hc32::gpio::{AlternateMode, AlternatePin, Level, OutputPin, Port};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embedded_hal::spi::{ErrorKind, ErrorType, Operation, SpiDevice};
use km003c_drivers::{
    calibration::{Calibration, Measurement},
    ina229::{Configuration, Ina229, Sample},
};

const BASE: usize = 0x4001c400;
const BUSY: u32 = 1 << 1;
const RX_FULL: u32 = 1 << 7;
const TX_EMPTY: u32 = 1 << 5;
const ERRORS: u32 = 0x1d;
const POLLS: usize = 200_000;
const RETAIN: *mut u32 = 0x200f0d40 as *mut u32;
const INIT_RETAIN: *mut u32 = 0x200f0c10 as *mut u32;
static CALIBRATION: Mutex<CriticalSectionRawMutex, Cell<Option<Calibration>>> =
    Mutex::new(Cell::new(None));

fn init_retain(index: usize, value: u32) {
    unsafe {
        INIT_RETAIN.add(index).write_volatile(value);
    }
}
pub fn init() {
    // Called before spawning either task, so flash/calibration publication
    // cannot race the measurement task's initialization.
    for index in 0..24 {
        retain(index, 0);
    }
    for index in 0..16 {
        init_retain(index, 0);
    }
    retain(0, 0x41444331);
    retain(1, 1);
    init_retain(0, 0x494e4931);
}
pub fn factory_settings(id: u16, block: &[u8; 96]) {
    retain(21, u32::from(id));
    retain(23, 0x3ff100);
    // A raw copy supports CRC/layout diagnosis after returning to native DFU.
    for (i, &byte) in block.iter().enumerate() {
        unsafe {
            (0x200f0da0 as *mut u8).add(i).write_volatile(byte);
        }
    }
    match Calibration::from_settings(block) {
        Ok(calibration) => {
            CALIBRATION.lock(|cell| cell.set(Some(calibration)));
            retain(22, 1);
            log::info!("Flash ID={id:04x}; factory calibration CRC valid: {calibration:?}");
        }
        Err(error) => {
            use km003c_drivers::calibration::Error;
            let code = match error {
                Error::Length => 2,
                Error::Checksum => 3,
                Error::LegacyFormat => 4,
                Error::Uncalibrated => 5,
                Error::Coefficient => 6,
            };
            retain(22, code);
            log::error!("Flash ID={id:04x}; calibration unavailable: {error:?}; keeping I0");
        }
    }
}
pub fn corrected(sample: Sample) -> Option<Measurement> {
    let current = uncalibrated_microamps(sample)?;
    CALIBRATION
        .lock(Cell::get)?
        .correct((sample.bus_nanovolts() / 1000) as u32, current)
}

fn retain(index: usize, value: u32) {
    // Separate from the wrapping USB log and panic/timeout snapshots. The native
    // loader preserves this bank across software reset, so ID/config/sample
    // evidence remains readable after the automatic return to DFU.
    unsafe {
        RETAIN.add(index).write_volatile(value);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Probing,
    WrongDevice { manufacturer: u16, device: u16 },
    Sample(Sample),
    BusError,
}
static STATE: Mutex<CriticalSectionRawMutex, Cell<State>> = Mutex::new(Cell::new(State::Probing));
pub fn snapshot() -> State {
    STATE.lock(Cell::get)
}
/// Native 2.0.7 at 0x6d78 returns a signed 24-bit result for ADCRANGE=1,
/// or four times that value for ADCRANGE=0. Its ADC task passes this directly
/// to the microamp calibration path at 0x17b1c. Thus each signed 20-bit count
/// represents 16 or 64 microamps *before* the per-device factory correction.
pub fn uncalibrated_microamps(sample: Sample) -> Option<i32> {
    if sample.shunt_cal != 0x1062 || sample.math_overflow() {
        return None;
    }
    let lsb = if sample.config & 0x10 != 0 {
        16000
    } else {
        64000
    };
    Some((sample.current_nanoamps(lsb) / 1000) as i32)
}
fn publish(state: State) {
    STATE.lock(|cell| cell.set(state));
}

#[derive(Debug)]
pub enum SpiError {
    Timeout,
    Status(u32),
    Faulted,
}
impl embedded_hal::spi::Error for SpiError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Status(s) if s & 4 != 0 => ErrorKind::ModeFault,
            Self::Status(s) if s & 1 != 0 => ErrorKind::Overrun,
            _ => ErrorKind::Other,
        }
    }
}

/// Board-specific SPI2 device with PA7 software CS and bounded polling.
pub struct Spi2Sensor {
    cs: OutputPin,
    _pins: [AlternatePin; 3],
    faulted: bool,
}
impl Spi2Sensor {
    /// # Safety
    /// Own SPI2, PA7/8/10 and PB0 exclusively, with PCLK1=96 MHz. No IRQ/DMA
    /// may touch SPI2. PA10 belongs to SPI2, not to this board's USB interface.
    pub unsafe fn new() -> Self {
        unsafe {
            // Native routine 0x6f9a: active-low FCG1 SPI2 gate, mode 1,
            // PCLK1/32 = 3 MHz. Keep the native CFG1 reserved-one/timing bits.
            let gate = 0x40048004 as *mut u32;
            gate.write_volatile(gate.read_volatile() & !(1 << 17));
            Self::write(4, 8); // Master, disabled while pins change.
            let cs = OutputPin::new(Port::A, 7, Level::High);
            let pins = [
                AlternatePin::new(Port::A, 8, 47, AlternateMode::Digital),
                AlternatePin::new(Port::A, 10, 44, AlternateMode::Digital),
                AlternatePin::new(Port::B, 0, 45, AlternateMode::Digital),
            ];
            Self::write(0x0c, 0xe10);
            Self::write(0x18, 0x40d); // 8-bit MSB-first; every byte is full-duplex.
            Self::write(4, 0x48);
            Self {
                cs,
                _pins: pins,
                faulted: false,
            }
        }
    }
    fn read(offset: usize) -> u32 {
        unsafe { ((BASE + offset) as *const u32).read_volatile() }
    }
    fn write(offset: usize, value: u32) {
        unsafe { ((BASE + offset) as *mut u32).write_volatile(value) }
    }
    fn wait(mask: u32, expected: u32) -> Result<(), SpiError> {
        for _ in 0..POLLS {
            let status = Self::read(0x14);
            if status & ERRORS != 0 {
                return Err(SpiError::Status(status));
            }
            if status & mask == expected {
                return Ok(());
            }
        }
        Err(SpiError::Timeout)
    }
    fn byte(value: u8) -> Result<u8, SpiError> {
        Self::wait(TX_EMPTY, TX_EMPTY)?;
        Self::write(0, u32::from(value));
        Self::wait(RX_FULL, RX_FULL)?;
        let byte = Self::read(0) as u8; // Drain RX for command/dummy bytes too.
        Self::wait(BUSY, 0)?;
        Ok(byte)
    }
}
impl ErrorType for Spi2Sensor {
    type Error = SpiError;
}
impl SpiDevice<u8> for Spi2Sensor {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), SpiError> {
        if self.faulted {
            return Err(SpiError::Faulted);
        }
        Self::wait(BUSY, 0)?;
        // Read and drain inherited receive data before a fresh transaction.
        if Self::read(0x14) & RX_FULL != 0 {
            let _ = Self::read(0);
        }
        self.cs.set_low();
        let result = (|| {
            for operation in operations {
                match operation {
                    Operation::Read(bytes) => {
                        for byte in bytes.iter_mut() {
                            *byte = Self::byte(0)?;
                        }
                    }
                    Operation::Write(bytes) => {
                        for &byte in *bytes {
                            Self::byte(byte)?;
                        }
                    }
                    Operation::Transfer(read, write) => {
                        for index in 0..read.len().max(write.len()) {
                            let byte = Self::byte(write.get(index).copied().unwrap_or(0))?;
                            if let Some(out) = read.get_mut(index) {
                                *out = byte;
                            }
                        }
                    }
                    Operation::TransferInPlace(bytes) => {
                        for byte in bytes.iter_mut() {
                            *byte = Self::byte(*byte)?;
                        }
                    }
                    Operation::DelayNs(ns) => {
                        // At 192 MHz, rounded-up whole microseconds are a
                        // conservative delay. SPI register operations add gaps.
                        let mut us = ns.div_ceil(1000);
                        while us != 0 {
                            cortex_m::asm::delay(192);
                            us -= 1;
                        }
                    }
                }
            }
            Self::wait(BUSY, 0)
        })();
        if result.is_err() {
            // Stop SCK before releasing CS; no implicit retry against uncertain
            // framing. Reinitialization requires dropping/recreating this owner.
            Self::write(4, Self::read(4) & !(1 << 6));
            self.faulted = true;
        }
        self.cs.set_high();
        result
    }
}
impl Drop for Spi2Sensor {
    fn drop(&mut self) {
        Self::write(4, Self::read(4) & !(1 << 6));
        self.cs.set_high();
    }
}

#[embassy_executor::task]
pub async fn task() {
    let mut sensor = Ina229::new(unsafe { Spi2Sensor::new() });
    let id = match sensor.identity() {
        Ok(id) => id,
        Err(e) => {
            log::error!("Measurement probe: {e:?}");
            retain(18, Spi2Sensor::read(0x14));
            retain(1, 0xe001);
            publish(State::BusError);
            return;
        }
    };
    log::info!("Measurement IDs: {id:?}");
    retain(2, u32::from(id.manufacturer));
    retain(3, u32::from(id.device));
    if !id.is_ina229() {
        retain(1, 0xe002);
        publish(State::WrongDevice {
            manufacturer: id.manufacturer,
            device: id.device,
        });
        return;
    }
    if cfg!(feature = "measurement-reset-test") {
        // Sensor-only reset reproduces its documented POR register defaults;
        // it is not claimed as a cold-start test of the MCU or the whole board.
        if let Err(error) = sensor.reset() {
            log::error!("INA229 reset: {error:?}");
            retain(1, 0xe005);
            publish(State::BusError);
            return;
        }
        embassy_time::Timer::after_millis(20).await;
        init_retain(15, 1);
    }
    let before = match sensor.configuration() {
        Ok(config) => config,
        Err(error) => {
            log::error!("Initial INA229 config: {error:?}");
            retain(1, 0xe003);
            publish(State::BusError);
            return;
        }
    };
    for (i, v) in [
        before.config,
        before.adc_config,
        before.shunt_cal,
        before.shunt_tempco,
    ]
    .into_iter()
    .enumerate()
    {
        init_retain(2 + i, u32::from(v));
    }
    if let Err(error) = sensor.configure(Configuration::NATIVE_199) {
        log::error!("INA229 configure: {error:?}");
        retain(1, 0xe006);
        publish(State::BusError);
        return;
    }
    match sensor.configuration() {
        Ok(config) => {
            if config != Configuration::NATIVE_199 {
                log::error!("INA229 config readback mismatch: {config:?}");
                retain(1, 0xe007);
                publish(State::BusError);
                return;
            }
            log::info!("Measurement config: {config:?}");
            for (index, value) in [
                config.config,
                config.adc_config,
                config.shunt_cal,
                config.shunt_tempco,
            ]
            .into_iter()
            .enumerate()
            {
                retain(4 + index, u32::from(value));
                init_retain(6 + index, u32::from(value));
            }
            retain(1, 2);
            init_retain(1, 2);
        }
        Err(e) => {
            log::error!("Measurement config: {e:?}");
            retain(18, Spi2Sensor::read(0x14));
            retain(1, 0xe003);
            publish(State::BusError);
            return;
        }
    }
    let mut count = 0u32;
    let (mut bus_min, mut bus_max) = (u32::MAX, 0u32);
    let (mut current_min, mut current_max) = (i32::MAX, i32::MIN);
    let fresh_deadline = embassy_time::Instant::now() + embassy_time::Duration::from_millis(100);
    loop {
        match sensor.sample() {
            Ok(sample) => {
                if count == 0 && sample.diagnostic & 2 == 0 {
                    if embassy_time::Instant::now() >= fresh_deadline {
                        log::error!("INA229 first conversion timeout");
                        retain(1, 0xe008);
                        publish(State::BusError);
                        return;
                    }
                    embassy_time::Timer::after_millis(2).await;
                    continue;
                }
                init_retain(1, 3);
                init_retain(12, u32::from(sample.diagnostic));
                count += 1;
                bus_min = bus_min.min(sample.bus_counts);
                bus_max = bus_max.max(sample.bus_counts);
                current_min = current_min.min(sample.current_counts);
                current_max = current_max.max(sample.current_counts);
                // Status=0 marks an incomplete record if the deadline resets
                // midway through publishing; write count/status last.
                retain(1, 0);
                for (index, value) in [
                    sample.bus_counts,
                    sample.shunt_counts as u32,
                    sample.current_counts as u32,
                    sample.temperature_counts as i32 as u32,
                    u32::from(sample.diagnostic),
                    bus_min,
                    bus_max,
                    current_min as u32,
                    current_max as u32,
                ]
                .into_iter()
                .enumerate()
                {
                    retain(9 + index, value);
                }
                retain(8, count);
                if let Some(current) = uncalibrated_microamps(sample) {
                    retain(19, current as u32);
                    retain(
                        20,
                        if sample.config & 0x10 != 0 {
                            16000
                        } else {
                            64000
                        },
                    );
                } else {
                    retain(20, 0);
                }
                retain(1, 3);
                if let Some(value) = corrected(sample) {
                    init_retain(10, value.voltage_microvolts);
                    init_retain(11, value.current_microamps as u32);
                    init_retain(13, 1);
                } else {
                    init_retain(13, 0);
                }
                log::info!(
                    "ADC bus={} nV shunt={} pV Iraw={} I0={:?} uA diag={:04x}",
                    sample.bus_nanovolts(),
                    sample.shunt_picovolts(),
                    sample.current_counts,
                    uncalibrated_microamps(sample),
                    sample.diagnostic
                );
                publish(State::Sample(sample));
            }
            Err(e) => {
                log::error!("ADC read: {e:?}");
                retain(18, Spi2Sensor::read(0x14));
                retain(1, 0xe004);
                publish(State::BusError);
                return;
            }
        }
        embassy_time::Timer::after_millis(250).await;
    }
}
