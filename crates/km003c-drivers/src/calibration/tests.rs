use super::*;
const BLOCK: &[u8; 96] = include_bytes!("../../tests/data/synthetic-calibration.bin");
fn checksum(block: &mut [u8; 96]) {
    let crc = crc32(&block[..92]);
    block[92..].copy_from_slice(&crc.to_le_bytes());
}
#[test]
fn matches_324_vectors_from_actual_native_instructions() {
    let c = Calibration::from_settings(BLOCK).unwrap();
    let mut count = 0;
    for row in include_str!("../../tests/data/native-calibration.csv").lines() {
        if row.starts_with('#') {
            continue;
        }
        let mut values = row.split(',').map(|s| s.parse::<i32>().unwrap());
        let v = values.next().unwrap() as u32;
        let i = values.next().unwrap();
        let expected = Measurement {
            voltage_microvolts: values.next().unwrap() as u32,
            current_microamps: values.next().unwrap(),
        };
        assert_eq!(c.correct(v, i), Some(expected), "input {v} uV, {i} uA");
        count += 1;
    }
    assert_eq!(count, 324);
}
#[test]
fn rejects_bad_data_instead_of_applying_defaults() {
    assert_eq!(Calibration::from_settings(&BLOCK[..95]), Err(Error::Length));
    let mut b = *BLOCK;
    b[28] ^= 1;
    assert_eq!(Calibration::from_settings(&b), Err(Error::Checksum));
    for (offset, value, error) in [
        (0, 2, Error::LegacyFormat),
        (0, 4, Error::Uncalibrated),
        (0x1c, 10001, Error::Coefficient),
        (0x30, 0, Error::Coefficient),
        (0x58, 500, Error::Coefficient),
    ] {
        let mut b = *BLOCK;
        b[offset..offset + 4].copy_from_slice(&i32::to_le_bytes(value));
        checksum(&mut b);
        assert_eq!(Calibration::from_settings(&b), Err(error));
    }
}
#[test]
fn constant_offset_form_and_checked_result() {
    let mut b = *BLOCK;
    for (i, v) in [-24i32, -6, -6, -6, -6].into_iter().enumerate() {
        b[28 + 4 * i..32 + 4 * i].copy_from_slice(&v.to_le_bytes());
    }
    checksum(&mut b);
    let c = Calibration::from_settings(&b).unwrap();
    assert_eq!(c.correct(5000000, 0).unwrap().voltage_microvolts, 4999976);
    assert_eq!(c.correct(0, 0).unwrap().voltage_microvolts, 0);
    assert_eq!(c.correct(0, i32::MIN), None);
}
