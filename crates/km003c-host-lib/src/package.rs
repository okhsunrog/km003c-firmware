//! Offline Qt .mencrypt container writer; wire validation is delegated to km003c-lib.
use aes::{
    Aes128,
    cipher::{BlockCipherEncrypt, KeyInit},
};
use anyhow::{Result, ensure};
use km003c_lib::{auth::MEMORY_READ_KEY, firmware::FirmwareImage};

pub fn package(binary: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        (0x460..=0x6fffe).contains(&binary.len()),
        "invalid application length"
    );
    ensure!(
        binary[0x420..0x424] == 0x4000u32.to_le_bytes(),
        "application must begin at 0x4000"
    );
    ensure!(
        binary[0x428..0x430] == [0xff; 8],
        "input must have uncommitted size/CRC metadata"
    );
    let mut identity = Vec::new();
    for offset in [0x430, 0x43c, 0x448] {
        let field = binary[offset..offset + 12]
            .split(|b| *b == 0)
            .next()
            .unwrap();
        ensure!(
            !field.is_empty() && field.len() < 12 && field.is_ascii(),
            "invalid embedded identity"
        );
        identity.push(std::str::from_utf8(field)?.to_owned());
    }
    ensure!(identity[0] == "KM003C", "unexpected model");
    let fields = vec![
        "Firmware file".to_owned(),
        "ChargerLab".to_owned(),
        "V1.0".to_owned(),
        identity[0].clone(),
        identity[1].clone(),
        identity[2].clone(),
        format!("{:08X}", crc32fast::hash(binary)),
        binary.len().to_string(),
        String::new(),
        "Custom Rust firmware".to_owned(),
    ];
    let mut output = Vec::new();
    output.extend_from_slice(&(fields.len() as u32).to_be_bytes());
    for field in fields {
        let encoded: Vec<u8> = field.encode_utf16().flat_map(u16::to_be_bytes).collect();
        output.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        output.extend_from_slice(&encoded);
    }
    let mut encrypted = binary.to_vec();
    encrypted.resize(binary.len().div_ceil(16) * 16, 0xff);
    let cipher = Aes128::new_from_slice(&*MEMORY_READ_KEY)?;
    for block in encrypted.as_chunks_mut::<16>().0 {
        cipher.encrypt_block(block.into());
    }
    output.extend_from_slice(&(encrypted.len() as u32).to_be_bytes());
    output.extend_from_slice(&encrypted);
    let image = FirmwareImage::parse(&output)?;
    image.validate_for_update()?;
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_roundtrip_matches_verified_installed_image() {
        let installed = include_bytes!("../../../hardware/rpc-cleanup/tested-image.bin");
        let mut raw = installed.to_vec();
        raw[0x428..0x430].fill(0xff);
        let encoded = package(&raw).unwrap();
        let image = FirmwareImage::parse(&encoded).unwrap();
        assert_eq!(image.installed_data().unwrap(), installed);
        // A changed vector is rejected even though the container gets a valid CRC.
        raw[..4].fill(0);
        assert!(package(&raw).is_err());
    }
    #[test]
    fn invalid_metadata_is_rejected_before_packaging() {
        assert!(package(&[]).is_err());
        assert!(package(&vec![0; 0x460]).is_err());
    }
}
