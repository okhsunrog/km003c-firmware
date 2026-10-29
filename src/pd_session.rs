//! Active sink session: choose CC from ADC while detached, lock it while
//! attached, and recreate the protocol state after a cable disconnect/flip.
use crate::{pd_buffered::Buffered, pd_tx, rpc_pd as telemetry};
use embassy_futures::select::{Either, select};
fn report(state: &str, input: u8) {
    telemetry::cc_state(state, input);
}

pub async fn run(rx: &mut Buffered, volts: u16, seconds: u16) -> Result<(), &'static str> {
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
            return Err("PD RX DMA did not quiesce");
        }
        pd_tx::attach(cc).await?;
        pd_tx::extend_lease(seconds);
        report("attached", cc.number());
        telemetry::sink_status("starting", u32::from(volts) * 1000);
        let outcome = select(crate::pd_sink::run(rx, volts), pd_tx::disconnected()).await;
        pd_tx::release_and_wait().await?;
        if !rx.stop() {
            return Err("PD RX DMA did not quiesce");
        }
        report("released", cc.number());
        match outcome {
            Either::First(()) => return Ok(()),
            Either::Second(()) => report("reorienting", cc.number()),
        }
    }
}
