//! Native-loader operations. Writes never retry an ambiguous data transfer.
use crate::rpc::Client;
use anyhow::{Context, Result, bail, ensure};
use km003c_lib::{
    DeviceSelector, KM003C, auth,
    firmware::{FirmwareImage, FirmwareUsbMode, InstalledFirmwareInfo, find_firmware_device},
};
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path, time::Duration};

const KNOWN_LOADER: &[u8] = include_bytes!("../../../fw/bootloader_hw2.1_00000004-00003fff.bin");
pub fn digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn save_new(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
/// Reuses the library's authenticated request/confirmation and AES decoder.
/// Loader confirmations can use their own response IDs; no APP-only reader is used.
pub async fn read(d: &mut KM003C, address: u32, size: usize) -> Result<Vec<u8>> {
    ensure!(
        size > 0 && size <= 0x80000 && address.checked_add(size as u32).is_some(),
        "invalid read range"
    );
    let mut output = Vec::with_capacity(size);
    while output.len() < size {
        let count = (size - output.len()).min(4096) as u32;
        let at = address + output.len() as u32;
        d.send_raw(&auth::build_memory_read_packet(at, count, 1))
            .await?;
        let reply = tokio::time::timeout(Duration::from_secs(5), d.receive_raw()).await??;
        ensure!(
            reply.len() == 20
                && auth::parse_memory_read_confirmation(&reply[4..]) == Some((at, count)),
            "invalid loader memory confirmation at {at:#x}"
        );
        let aligned = count.div_ceil(16) * 16;
        let mut encrypted = Vec::new();
        while encrypted.len() < aligned as usize {
            encrypted.extend(tokio::time::timeout(Duration::from_secs(5), d.receive_raw()).await??);
        }
        ensure!(
            encrypted.len() == aligned as usize,
            "unexpected loader transfer length"
        );
        let plain = auth::decrypt_memory_read_response(&encrypted)
            .context("invalid encrypted memory data")?;
        output.extend_from_slice(&plain[..count as usize]);
    }
    Ok(output)
}
pub async fn open(selector: DeviceSelector) -> Result<(KM003C, DeviceSelector)> {
    let found = find_firmware_device(&selector).await?;
    let stable = found.port_selector.unwrap_or(selector);
    ensure!(
        found.mode == FirmwareUsbMode::Dfu,
        "native DFU required; run the loader command or hold Exit during power-on"
    );
    let mut d = KM003C::open_maintenance(stable.clone()).await?;
    let info = d.get_bootloader_info().await?;
    ensure!(
        info.model == "KM003C" && info.hw_version == "2.1",
        "unsupported device: {info:?}"
    );
    Ok((d, stable))
}
pub async fn enter(selector: DeviceSelector) -> Result<DeviceSelector> {
    let found = find_firmware_device(&selector).await?;
    let stable = found.port_selector.unwrap_or(selector);
    if found.mode != FirmwareUsbMode::Dfu {
        // Only our RPC app is rebooted automatically. Stock protocol operations
        // remain in km003c-lib and are not guessed for unknown USB interfaces.
        Client::connect(stable.clone()).await?.loader().await?;
    }
    Ok(stable)
}
fn installed_info(bytes: &[u8]) -> Result<InstalledFirmwareInfo> {
    ensure!(bytes.len() >= 0x460, "truncated application");
    let mut info = auth::DeviceInfo::default();
    info.parse_firmware_info(&bytes[0x420..0x460]);
    Ok(InstalledFirmwareInfo {
        version: info.fw_version,
        date: info.fw_date,
        size: u32::from_le_bytes(bytes[0x428..0x42c].try_into()?) as usize,
        crc32: u32::from_le_bytes(bytes[0x42c..0x430].try_into()?),
    })
}
pub async fn backup(selector: DeviceSelector, path: &Path) -> Result<()> {
    ensure!(
        !path.exists(),
        "backup already exists; refusing to overwrite it"
    );
    let (mut d, _) = open(selector).await?;
    let metadata = read(&mut d, 0x4420, 64).await?;
    let size = u32::from_le_bytes(metadata[8..12].try_into()?) as usize;
    ensure!(
        (0x460..=0x6fffe).contains(&size),
        "invalid installed application size"
    );
    let data = read(&mut d, 0x4000, size).await?;
    installed_info(&data)?.validate_application(&data)?;
    save_new(path, &data)
}
pub async fn verify(selector: DeviceSelector, image: &FirmwareImage) -> Result<()> {
    image.validate_for_update()?;
    let (mut d, _) = open(selector).await?;
    ensure!(
        read(&mut d, 4, KNOWN_LOADER.len()).await? == KNOWN_LOADER,
        "loader differs from verified HW2.1 reference"
    );
    ensure!(
        read(&mut d, 0x4000, image.data().len()).await? == image.installed_data()?,
        "application readback differs"
    );
    Ok(())
}
/// On any failure the device stays in DFU; application start is a separate step.
pub async fn flash(
    selector: DeviceSelector,
    image: &FirmwareImage,
    evidence: &Path,
) -> Result<DeviceSelector> {
    image.validate_for_update()?;
    std::fs::create_dir(evidence)
        .context("evidence directory must be new; its parent must exist")?;
    let selector = enter(selector).await?;
    let (mut d, stable) = open(selector).await?;
    let before = read(&mut d, 4, KNOWN_LOADER.len()).await?;
    ensure!(
        before == KNOWN_LOADER,
        "loader differs from verified HW2.1 reference"
    );
    save_new(&evidence.join("loader-before.bin"), &before)?;
    // Persist provenance before the first erase.
    save_new(&evidence.join("before.json"),serde_json::to_string_pretty(&serde_json::json!({"image_sha256":digest(image.data()),"loader_sha256":digest(&before),"selector":format!("{stable:?}"),"model":"KM003C","hw_version":"2.1"}))?.as_bytes())?;
    d.update_firmware(image, |p| {
        eprintln!("{:?}: {}/{}", p.stage, p.bytes_written, p.total_bytes)
    })
    .await?;
    let actual = read(&mut d, 0x4000, image.data().len()).await?;
    save_new(&evidence.join("application-readback.bin"), &actual)?;
    let after = read(&mut d, 4, KNOWN_LOADER.len()).await?;
    save_new(&evidence.join("loader-after.bin"), &after)?;
    ensure!(before == after, "loader changed during update");
    ensure!(
        actual == image.installed_data()?,
        "application readback differs"
    );
    save_new(&evidence.join("verified.json"),serde_json::to_string_pretty(&serde_json::json!({"application_sha256":digest(&actual),"loader_unchanged":true,"readback_verified":true}))?.as_bytes())?;
    Ok(stable)
}
pub async fn start(selector: DeviceSelector) -> Result<()> {
    let (mut d, stable) = open(selector).await?;
    d.jump_to_application().await?;
    drop(d);
    // Enumeration is asynchronous; do not report a usable application early.
    Client::connect(stable).await?.status().await?;
    Ok(())
}

pub async fn diagnostics(selector: DeviceSelector, output: &Path) -> Result<()> {
    if output.exists() {
        bail!("diagnostics directory already exists");
    }
    let (mut d, _) = open(selector).await?;
    std::fs::create_dir(output)?;
    let retention = read(&mut d, 0x200f0000, 0x1000).await?;
    save_new(&output.join("retention.bin"), &retention)?;
    let cursor = u32::from_le_bytes(retention[0xc00..0xc04].try_into()?) as usize;
    let trace: Vec<u8> = (cursor.saturating_sub(3072)..cursor)
        .map(|i| retention[i % 3072])
        .collect();
    save_new(&output.join("trace.log"), &trace)?;
    let status = u32::from_le_bytes(retention[0xfcc..0xfd0].try_into()?);
    let panic = if status == 0xbad00002 {
        Some(
            String::from_utf8_lossy(retention[0xe00..0xf00].split(|b| *b == 0).next().unwrap())
                .into_owned(),
        )
    } else {
        None
    };
    save_new(&output.join("status.json"),serde_json::to_string_pretty(&serde_json::json!({"return_status":format!("{status:#010x}"),"panic":panic,"trace_cursor":cursor}))?.as_bytes())?;
    save_new(
        &output.join("loader.bin"),
        &read(&mut d, 4, KNOWN_LOADER.len()).await?,
    )?;

    Ok(())
}

pub fn parse_image(bytes: &[u8]) -> Result<FirmwareImage> {
    let image = FirmwareImage::parse(bytes)?;
    image.validate_for_update()?;
    Ok(image)
}
