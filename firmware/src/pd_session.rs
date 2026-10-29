//! Active sink session: choose CC from ADC while detached, lock it while
//! attached, and recreate the protocol state after a cable disconnect/flip.
use crate::{pd_buffered::Buffered, pd_service as telemetry, pd_tx};
use embassy_futures::select::{Either, select};
use km003c_interface::PdEndReason;
fn report(state: &str, input: u8) {
    telemetry::cc_state(state, input);
}

pub async fn run(rx: &mut Buffered, volts: u16, seconds: u16) -> Result<(), PdEndReason> {
    let mut waiting = false;
    loop {
        let cc = match pd_tx::prepare().await {
            Ok(cc) => cc,
            Err(_) => {
                if !waiting {
                    report("waiting", 0);
                    waiting = true;
                }
                embassy_time::Timer::after_millis(50).await;
                continue;
            }
        };
        waiting = false;
        if !rx.start(cc) {
            return Err(PdEndReason::RxStartFailed);
        }
        pd_tx::attach(cc)
            .await
            .map_err(|_| PdEndReason::AttachFailed)?;
        pd_tx::extend_lease(seconds);
        report("attached", cc.number());
        telemetry::sink_status("starting", u32::from(volts) * 1000);
        let outcome = select(crate::pd_sink::run(rx, volts), pd_tx::disconnected()).await;
        pd_tx::release_and_wait()
            .await
            .map_err(|_| PdEndReason::ReleaseFailed)?;
        if !rx.stop() {
            return Err(PdEndReason::RxStopFailed);
        }
        report("released", cc.number());
        match outcome {
            Either::First(result) => return result.map_err(|_| PdEndReason::StackFailed),
            Either::Second(()) => report("reorienting", cc.number()),
        }
    }
}
