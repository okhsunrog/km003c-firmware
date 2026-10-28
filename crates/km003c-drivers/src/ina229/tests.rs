use super::*;
use embedded_hal::spi::{ErrorKind, ErrorType, Operation};
extern crate std;
use std::vec::Vec;

#[derive(Debug, PartialEq, Eq)]
struct BusError;
impl embedded_hal::spi::Error for BusError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}
#[derive(Default)]
struct Mock {
    writes: Vec<Vec<u8>>,
    wrong_id: bool,
    fail: bool,
}
impl ErrorType for Mock {
    type Error = BusError;
}
impl SpiDevice for Mock {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), BusError> {
        if self.fail {
            return Err(BusError);
        }
        match operations {
            [Operation::Write(bytes)] => {
                assert_eq!(bytes.len(), 3);
                assert_eq!(bytes[0] & 3, 0);
                self.writes.push(bytes.to_vec());
            }
            [Operation::Write(command), Operation::Read(out)] => {
                assert_eq!(command.len(), 1);
                assert_eq!(command[0] & 3, 1);
                let response: &[u8] = match command[0] >> 2 {
                    0 => &[0x01, 0x70],
                    1 => &[0xfd, 0xb0],
                    2 => &[0x10, 0x62],
                    3 => &[0, 0x1e],
                    4 => &[0xff, 0xff, 0xf0], // -1 shunt count
                    5 => &[0x06, 0x40, 0x00], // 25600 counts = 5 V
                    6 => &[0x0c, 0x80],       // 3200 counts = 25 C
                    7 => &[0x80, 0, 0],       // most negative signed 20-bit current
                    0x0b => &[0x02, 0x03],
                    0x3e if self.wrong_id => &[0xff, 0xff],
                    0x3e => &[0x54, 0x49],
                    0x3f => &[0x22, 0x91],
                    _ => panic!("unexpected command"),
                };
                out.copy_from_slice(response);
            }
            _ => panic!("address and data must share a single CS transaction"),
        }
        Ok(())
    }
}
#[test]
fn sample_width_sign_and_physical_units() {
    let mut device = Ina229::new(Mock::default());
    let sample = device.sample().unwrap();
    assert_eq!(sample.bus_nanovolts(), 5_000_000_000);
    assert_eq!(sample.shunt_counts, -1);
    assert_eq!(sample.shunt_picovolts(), -78125);
    assert_eq!(sample.current_counts, -524288);
    assert_eq!(sample.temperature_millicelsius(), 25000);
    assert!(sample.math_overflow());
    assert_eq!(sample.current_nanoamps(16000), -8_388_608_000);
    let wide = Sample {
        config: 0,
        temperature_counts: -128,
        ..sample
    };
    assert_eq!(wide.shunt_picovolts(), -312500);
    assert_eq!(wide.temperature_millicelsius(), -1000);
    assert!(device.release().writes.is_empty());
}
#[test]
fn full_signed_range() {
    assert_eq!(signed20(0x7ffff0), 524287);
    assert_eq!(signed20(0x800000), -524288);
    assert_eq!(signed20(0xfffff0), -1);
    assert_eq!(signed20(0), 0);
}

#[test]
fn reset_checks_identity_before_writing_por_bit() {
    let mut device = Ina229::new(Mock::default());
    device.reset().unwrap();
    assert_eq!(device.release().writes, [[0, 0x80, 0]]);
    let mut device = Ina229::new(Mock {
        wrong_id: true,
        ..Mock::default()
    });
    assert!(matches!(device.reset(), Err(Error::UnexpectedIdentity(_))));
    assert!(device.release().writes.is_empty());
}
#[test]
fn probe_before_writes_and_start_last() {
    let mut device = Ina229::new(Mock::default());
    assert!(device.identity().unwrap().is_ina229());
    assert_eq!(device.configuration().unwrap(), Configuration::NATIVE_199);
    device.configure(Configuration::NATIVE_199).unwrap();
    assert_eq!(
        device.release().writes,
        [
            [0, 1, 0x70],
            [8, 0x10, 0x62],
            [12, 0, 0x1e],
            [4, 0xfd, 0xb0],
        ]
    );
}
#[test]
fn wrong_identity_invalid_config_and_bus_error_never_write() {
    let mut device = Ina229::new(Mock {
        wrong_id: true,
        ..Mock::default()
    });
    assert!(matches!(
        device.configure(Configuration::NATIVE_199),
        Err(Error::UnexpectedIdentity(_))
    ));
    assert!(device.release().writes.is_empty());
    for config in [
        Configuration {
            config: 0x8000,
            ..Configuration::NATIVE_199
        },
        Configuration {
            config: 1,
            ..Configuration::NATIVE_199
        },
        Configuration {
            shunt_cal: 0,
            ..Configuration::NATIVE_199
        },
        Configuration {
            shunt_tempco: 0x4000,
            ..Configuration::NATIVE_199
        },
    ] {
        let mut device = Ina229::new(Mock::default());
        assert_eq!(device.configure(config), Err(Error::InvalidConfiguration));
        assert!(device.release().writes.is_empty());
    }
    let mut device = Ina229::new(Mock {
        fail: true,
        ..Mock::default()
    });
    assert_eq!(device.sample(), Err(Error::Bus(BusError)));
}
