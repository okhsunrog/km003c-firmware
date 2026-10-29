//! Small native-bulk client; JSON export belongs on this side of the wire.
use km003c_interface::{
    ControlPd, EnterLoader, GetStatus, Measurements, PdControl, PdEvents, SetStream,
};
use postcard_rpc::{header::VarSeqKind, host_client::HostClient, standard_icd::WireError};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HostClient::<WireError>::try_new_raw_nusb(
        |d| {
            d.vendor_id() == 0x5fc9
                && d.product_id() == 0x0063
                && d.product_string() == Some("KM003C Rust RPC")
        },
        "error",
        8,
        VarSeqKind::Seq2,
    )
    .await?;
    let status = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.send_resp::<GetStatus>(&()),
    )
    .await??;
    println!("{status:?}");
    if std::env::args().nth(1).as_deref() == Some("loader") {
        // Reset disconnects USB before a reply can arrive.
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client.send_resp::<EnterLoader>(&()),
        )
        .await;
        return Ok(());
    }
    let mut sub = client.subscribe_exclusive::<Measurements>(16).await?;
    let mut pd = client.subscribe_exclusive::<PdEvents>(64).await?;
    let mut logs = client
        .subscribe_exclusive::<postcard_rpc::standard_icd::LoggingTopic>(16)
        .await?;
    client.send_resp::<SetStream>(&true).await?;
    let args: Vec<_> = std::env::args().collect();
    let sink = args.get(1).is_some_and(|s| s == "sink");
    if sink {
        let volts = args.get(2).ok_or("usage: sink VOLTS")?.parse()?;
        let result = client
            .send_resp::<ControlPd>(&PdControl::Sink { volts, seconds: 10 })
            .await?;
        result.map_err(|e| format!("PD command rejected: {e:?}"))?;
    }
    let end = tokio::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        tokio::select! {
            _=tokio::time::sleep_until(end)=>break,
            msg=sub.recv()=>println!("measurement {msg:?}"),
            msg=pd.recv()=>println!("pd {msg:?}"),
            msg=logs.recv()=>println!("log {msg:?}"),
        }
    }
    if sink {
        let _ = client.send_resp::<ControlPd>(&PdControl::Stop).await?;
        let end = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            tokio::select! { _=tokio::time::sleep_until(end)=>break, msg=pd.recv()=>println!("pd {msg:?}"), msg=sub.recv()=>println!("measurement {msg:?}") }
        }
    }
    client.send_resp::<SetStream>(&false).await?;
    Ok(())
}
