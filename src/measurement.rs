//! Optional, read-only SPI2 measurement task. No sensor configuration writes.
//! Enable explicitly with `--features measurement` after reviewing board pins.
use core::cell::Cell;
use embassy_hc32::gpio::{AlternateMode, AlternatePin, Level, OutputPin, Port};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embedded_hal::spi::{ErrorKind, ErrorType, Operation, SpiDevice};
use km003c_drivers::ina229::{Ina229, Sample};

const BASE: usize = 0x4001c400;
const BUSY: u32 = 1 << 1;
const RX_FULL: u32 = 1 << 7;
const TX_EMPTY: u32 = 1 << 5;
const ERRORS: u32 = 0x1d;
const POLLS: usize = 200_000;

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
            publish(State::BusError);
            return;
        }
    };
    log::info!("Measurement IDs: {id:?}");
    if !id.is_ina229() {
        publish(State::WrongDevice {
            manufacturer: id.manufacturer,
            device: id.device,
        });
        return;
    }
    log::info!("Measurement config: {:?}", sensor.configuration());
    loop {
        match sensor.sample() {
            Ok(sample) => {
                log::info!(
                    "ADC bus={} nV shunt={} pV Iraw={} diag={:04x}",
                    sample.bus_nanovolts(),
                    sample.shunt_picovolts(),
                    sample.current_counts,
                    sample.diagnostic
                );
                publish(State::Sample(sample));
            }
            Err(e) => {
                log::error!("ADC read: {e:?}");
                publish(State::BusError);
                return;
            }
        }
        embassy_time::Timer::after_millis(250).await;
    }
}
