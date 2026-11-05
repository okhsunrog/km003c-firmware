use anyhow::{Result, anyhow, ensure};
use km003c_interface::{
    ControlPd, EnterLoader, FixedProfile, GetMeasurement, GetPdStatus, GetStatus, Measurement,
    PdControl, SetStream, SetStreamInterval, Status,
};
use km003c_lib::{
    DeviceSelector,
    firmware::{FirmwareUsbMode, find_firmware_device},
};
use postcard_rpc::{header::VarSeqKind, host_client::HostClient, standard_icd::WireError};
use std::time::Duration;

pub type Rpc = HostClient<WireError>;

pub struct Client {
    pub rpc: Rpc,
    /// The physical port remains stable when firmware and loader re-enumerate.
    pub selector: DeviceSelector,
    pub protocol_version: u16,
}

impl Client {
    pub async fn connect(selector: DeviceSelector) -> Result<Self> {
        let found = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match find_firmware_device(&selector).await {
                    Ok(found) if found.product_string.as_deref() == Some("KM003C Rust RPC") => {
                        return Ok(found);
                    }
                    Ok(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                    Err(km003c_lib::error::KMError::DeviceNotFound) => {
                        tokio::time::sleep(Duration::from_millis(100)).await
                    }
                    Err(error) => return Err(error),
                }
            }
        })
        .await
        .map_err(|_| anyhow!("selected device did not enumerate as the RPC application"))??;
        let stable = found.port_selector.unwrap_or(selector);
        let filter = stable.clone();
        let rpc = HostClient::try_new_raw_nusb(
            move |d| {
                d.vendor_id() == 0x5fc9
                    && d.product_id() == 0x0063
                    && d.product_string() == Some("KM003C Rust RPC")
                    && match &filter {
                        DeviceSelector::First => true,
                        DeviceSelector::SerialNumber(s) => d.serial_number() == Some(s.as_str()),
                        DeviceSelector::BusAddress {
                            bus_id,
                            device_address,
                        } => d.bus_id() == bus_id && d.device_address() == *device_address,
                        DeviceSelector::PortPath { bus_id, port_chain } => {
                            d.bus_id() == bus_id && d.port_chain() == port_chain
                        }
                    }
            },
            "error",
            8,
            VarSeqKind::Seq2,
        )
        .await
        .map_err(|s| anyhow!(s))?;
        let status =
            tokio::time::timeout(Duration::from_secs(3), rpc.send_resp::<GetStatus>(&())).await??;
        Ok(Self {
            rpc,
            selector: stable,
            protocol_version: status.protocol_version,
        })
    }

    pub async fn status(&self) -> Result<Status> {
        Ok(
            tokio::time::timeout(Duration::from_secs(3), self.rpc.send_resp::<GetStatus>(&()))
                .await??,
        )
    }

    pub async fn pd_status(&self) -> Result<km003c_interface::PdSnapshot> {
        self.require_protocol()?;
        Ok(tokio::time::timeout(
            Duration::from_secs(1),
            self.rpc.send_resp::<GetPdStatus>(&()),
        )
        .await??)
    }

    pub async fn stream(&self, enabled: bool) -> Result<()> {
        tokio::time::timeout(
            Duration::from_secs(3),
            self.rpc.send_resp::<SetStream>(&enabled),
        )
        .await??;
        Ok(())
    }

    fn require_protocol(&self) -> Result<()> {
        ensure!(
            self.protocol_version == km003c_interface::PROTOCOL_VERSION,
            "requires firmware protocol {}; update the device first (device reports {})",
            km003c_interface::PROTOCOL_VERSION,
            self.protocol_version
        );
        Ok(())
    }

    pub async fn stream_interval(&self, ms: u16) -> Result<()> {
        self.require_protocol()?;
        ensure!(
            km003c_interface::valid_stream_interval(ms),
            "measurement interval must be 20..5000 ms"
        );
        tokio::time::timeout(
            Duration::from_secs(3),
            self.rpc.send_resp::<SetStreamInterval>(&ms),
        )
        .await??
        .map_err(|e| anyhow!("interval rejected: {e:?}"))
    }

    pub async fn measurement(&self) -> Result<Option<Measurement>> {
        self.require_protocol()?;
        Ok(tokio::time::timeout(
            Duration::from_secs(3),
            self.rpc.send_resp::<GetMeasurement>(&()),
        )
        .await??)
    }

    pub async fn control(&self, command: PdControl) -> Result<u32> {
        ensure!(
            self.protocol_version == km003c_interface::PROTOCOL_VERSION,
            "PD control requires firmware protocol {}; update the device first",
            km003c_interface::PROTOCOL_VERSION
        );
        validate_control(command)?;
        let response = tokio::time::timeout(
            Duration::from_secs(3),
            self.rpc.send_resp::<ControlPd>(&command),
        )
        .await??;
        response.map_err(|e| anyhow!("PD command rejected: {e:?}"))
    }

    /// A reset may interrupt its own reply. Success is DFU on the same port,
    /// never merely a timeout or a disconnected RPC handle.
    pub async fn loader(&self) -> Result<()> {
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            self.rpc.send_resp::<EnterLoader>(&()),
        )
        .await;
        if matches!(response, Ok(Ok(Err(())))) {
            anyhow::bail!("device refused loader entry: CC release was not confirmed");
        }
        wait_dfu(&self.selector).await
    }
}

pub fn validate_control(command: PdControl) -> Result<()> {
    match command {
        PdControl::Sink {
            volts,
            seconds,
            current_ma,
        } => ensure!(
            (FixedProfile { volts, current_ma }).valid() && (1..=600).contains(&seconds),
            "sink requires 5/9/12/15/20 V, 100..3000 mA in 10 mA steps, and 1..600 seconds"
        ),
        PdControl::SetProfile { volts, current_ma } => ensure!(
            (FixedProfile { volts, current_ma }).valid(),
            "profile requires 5/9/12/15/20 V and 100..3000 mA in 10 mA steps"
        ),
        PdControl::Monitor { input: Some(n) } => {
            ensure!([1, 2].contains(&n), "CC input must be 1, 2 or auto")
        }
        _ => {}
    }
    Ok(())
}

pub async fn wait_dfu(selector: &DeviceSelector) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Ok(d) = find_firmware_device(selector).await
                && d.mode == FirmwareUsbMode::Dfu
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| anyhow!("device did not enter native DFU on the selected port"))
}
