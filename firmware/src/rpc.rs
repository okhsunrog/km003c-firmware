//! Native USB bulk RPC. Each publish has a bound; metering never waits for a host.
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb_synopsys_otg::Driver;
use km003c_interface::*;
use postcard_rpc::server::impls::embassy_usb_v0_6::dispatch_impl::{
    WireSpawnImpl, WireStorage, WireTxImpl,
};
use postcard_rpc::{
    define_dispatch,
    header::VarHeader,
    server::{Dispatch, Sender, SpawnContext},
};
type Tx = WireTxImpl<CriticalSectionRawMutex, Driver<'static>>;
static STREAM: AtomicBool = AtomicBool::new(false);
type LogLine = heapless::String<192>;
static LOGS: embassy_sync::channel::Channel<CriticalSectionRawMutex, LogLine, 8> =
    embassy_sync::channel::Channel::new();
/// Synchronous producers never wait for USB or a mutex held by the sender.
pub fn log(line: LogLine) {
    let _ = LOGS.try_send(line);
}
pub struct Context;
impl SpawnContext for Context {
    type SpawnCtxt = ();
    fn spawn_ctxt(&mut self) {}
}
fn pd_control(_: &mut Context, _: VarHeader, command: PdControl) -> ControlResult {
    crate::pd_service::control(command)
}
fn status(_: &mut Context, _: VarHeader, _: ()) -> Status {
    Status {
        protocol_version: PROTOCOL_VERSION,
        uptime_ms: embassy_time::Instant::now().as_millis(),
        hclk_hz: 192_000_000,
    }
}
fn stream(_: &mut Context, _: VarHeader, enabled: bool) {
    STREAM.store(enabled, Ordering::Relaxed);
}
async fn loader(_: &mut Context, _: VarHeader, _: ()) -> LoaderResult {
    crate::enter_loader(0x600d0002).await.map_err(|_| ())
}
define_dispatch! {
    app: App;
    spawn_fn: spawn_fn;
    tx_impl: Tx;
    spawn_impl: WireSpawnImpl;
    context: Context;
    endpoints: {
        list: ENDPOINTS;
        | EndpointTy | kind | handler |
        | ---------- | ---- | ------- |
        | GetStatus | blocking | status |
        | SetStream | blocking | stream |
        | EnterLoader | async | loader |
        | ControlPd | blocking | pd_control |

    };
    topics_in: {
        list: TOPICS_IN;
        | TopicTy | kind | handler |
        | ------- | ---- | ------- |
    };
    topics_out: { list: TOPICS_OUT; };
}
pub async fn run(driver: Driver<'static>, spawner: embassy_executor::Spawner) {
    static STORAGE: WireStorage<CriticalSectionRawMutex, Driver<'static>> = WireStorage::new();
    static TX: static_cell::StaticCell<[u8; 512]> = static_cell::StaticCell::new();
    static RX: static_cell::StaticCell<[u8; 512]> = static_cell::StaticCell::new();
    let mut config = embassy_usb::Config::new(0x5fc9, 0x0063);
    config.manufacturer = Some("KM003C experimental");
    config.product = Some("KM003C Rust RPC");
    config.serial_number = Some("rust-bringup");
    let (mut usb, tx, rx) = STORAGE.init(driver, config, TX.init([0; 512]), 64);
    let app = App::new(Context, WireSpawnImpl::from(spawner));
    let key = app.min_key_len();
    let mut server =
        postcard_rpc::server::Server::new(tx, rx, RX.init([0; 512]).as_mut_slice(), app, key);
    let sender = server.sender();
    let serve = async {
        loop {
            let _ = server.run().await;
            let _ = crate::pd_service::control(PdControl::Stop);
            STREAM.store(false, Ordering::Relaxed);
            embassy_time::Timer::after_millis(100).await;
        }
    };
    let logging = async {
        loop {
            let line = LOGS.receive().await;
            let _ = embassy_time::with_timeout(
                embassy_time::Duration::from_millis(20),
                sender.log_str(&line),
            )
            .await;
        }
    };
    let pd = async {
        {
            let mut seq = 0i32;
            loop {
                let event = crate::pd_service::EVENTS.receive().await;
                let _ = embassy_time::with_timeout(
                    embassy_time::Duration::from_millis(20),
                    sender.publish::<PdEvents>(postcard_rpc::header::VarSeq::Seq4(seq), &event),
                )
                .await;
                seq = seq.wrapping_add(1);
            }
        }
    };
    embassy_futures::join::join5(usb.run(), serve, publish(sender.clone()), logging, pd).await;
}
async fn publish(sender: Sender<Tx>) {
    let mut seq = 0u32;
    loop {
        embassy_time::Timer::after_millis(250).await;
        if !STREAM.load(Ordering::Relaxed) {
            continue;
        }
        if let crate::measurement::State::Sample(sample) = crate::measurement::snapshot() {
            let corrected = crate::measurement::corrected(sample);
            let msg = Measurement {
                uptime_ms: embassy_time::Instant::now().as_millis(),
                voltage_uv: corrected
                    .map(|s| s.voltage_microvolts)
                    .unwrap_or((sample.bus_nanovolts() / 1000) as u32),
                current_ua: corrected
                    .map(|s| s.current_microamps)
                    .or_else(|| crate::measurement::uncalibrated_microamps(sample)),
                temperature_mc: sample.temperature_millicelsius(),
                calibrated: corrected.is_some(),
                diagnostic: sample.diagnostic,
                lines_uv: crate::lines::snapshot().map(|s| s.uv),
            };
            let _ = embassy_time::with_timeout(
                embassy_time::Duration::from_millis(20),
                sender
                    .publish::<Measurements>(postcard_rpc::header::VarSeq::Seq4(seq as i32), &msg),
            )
            .await;
            seq = seq.wrapping_add(1);
        }
    }
}
