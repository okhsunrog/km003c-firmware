//! Typed capture and timestamped command records; JSON is a host output format.
use crate::{
    maneuver::{Maneuver, Step},
    rpc::Client,
};
use anyhow::{Context, Result, bail};
use km003c_interface::{Measurement, Measurements, PdControl, PdEvent, PdEvents, PdState};
use postcard_rpc::standard_icd::LoggingTopic;
use serde::Serialize;
use std::{
    io::{BufWriter, Write},
    path::Path,
    time::Duration,
};
use tokio::time::Instant;
#[derive(Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Record<'a> {
    Maneuver(&'a Maneuver),
    Measurement(Measurement),
    Pd(PdEvent),
    PdSnapshot(km003c_interface::PdSnapshot),
    Log(String),
    Command { index: usize, command: PdControl },
    Accepted { index: usize, device_command: u32 },
    Error(String),
    Finished,
}
struct Recorder {
    writer: Box<dyn Write + Send>,
    start: Instant,
}
impl Recorder {
    fn new(path: Option<&Path>) -> Result<Self> {
        let writer: Box<dyn Write + Send> = if let Some(path) = path {
            Box::new(BufWriter::new(
                std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path)?,
            ))
        } else {
            Box::new(std::io::stdout())
        };
        Ok(Self {
            writer,
            start: Instant::now(),
        })
    }
    fn emit(&mut self, event: Record<'_>) -> Result<()> {
        serde_json::to_writer(
            &mut self.writer,
            &serde_json::json!({"host_elapsed_us":self.start.elapsed().as_micros(),"event":event}),
        )?;
        writeln!(self.writer)?;
        self.writer.flush()?;
        Ok(())
    }
}
pub async fn run(client: &Client, plan: &Maneuver, output: Option<&Path>) -> Result<()> {
    plan.validate()?;
    let mut recorder = Recorder::new(output)?;
    recorder.emit(Record::Maneuver(plan))?;
    let mut measurements = client.rpc.subscribe_exclusive::<Measurements>(32).await?;
    let mut pd = client.rpc.subscribe_exclusive::<PdEvents>(64).await?;
    let mut logs = client.rpc.subscribe_exclusive::<LoggingTopic>(32).await?;
    let result:Result<()>=async {
        client.stream(true).await?;
        let start=Instant::now();let end=start+Duration::from_millis(plan.duration_ms);let mut index=0;
        loop {
            let due=plan.steps.get(index).map_or(end,|s|start+Duration::from_millis(s.at_ms));
            tokio::select! {
                _=tokio::signal::ctrl_c()=>break,
                _=tokio::time::sleep_until(end)=>break,
                _=tokio::time::sleep_until(due), if index<plan.steps.len()=>{
                    let Step{command,..}=plan.steps[index];
                    recorder.emit(Record::Command{index,command})?;
                    let device_command=client.control(command).await?;
                    recorder.emit(Record::Accepted{index,device_command})?;
                    if matches!(command,PdControl::Stop) {wait_detached(client,&mut pd,device_command,&mut recorder).await?;}
                    index+=1;
                }
                msg=measurements.recv()=>recorder.emit(Record::Measurement(msg.context("measurement stream closed")?))?,
                msg=pd.recv()=>recorder.emit(Record::Pd(msg.context("PD stream closed")?))?,
                msg=logs.recv()=>recorder.emit(Record::Log(msg.context("logging stream closed")?))?,
            }
        }
        Ok(())
    }.await;
    if let Err(e) = &result {
        let _ = recorder.emit(Record::Error(e.to_string()));
    }
    // Cleanup is attempted even if recording, decoding or a timeline command failed.
    let stop = client.control(PdControl::Stop).await;
    let detached = if let Ok(stop_id) = &stop {
        wait_detached(client, &mut pd, *stop_id, &mut recorder).await
    } else {
        Ok(())
    };
    let stream_off = client.stream(false).await;
    let cleanup = stop.map(|_| ()).and(detached).and(stream_off);
    if let Err(error) = result {
        if let Err(cleanup_error) = cleanup {
            return Err(error.context(format!("cleanup failed: {cleanup_error:#}")));
        }
        return Err(error);
    }
    cleanup?;
    recorder.emit(Record::PdSnapshot(client.pd_status().await?))?;
    recorder.emit(Record::Finished)
}

async fn wait_detached(
    client: &Client,
    pd: &mut postcard_rpc::host_client::Subscription<PdEvent>,
    id: u32,
    recorder: &mut Recorder,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut events_open=true;
        loop {
            let event = tokio::select! {
                msg=pd.recv(), if events_open=>match msg {Some(event)=>event,None=>{events_open=false;continue;}},
                _=tokio::time::sleep(Duration::from_millis(100))=>{
                    let status=client.pd_status().await?;
                    match confirmed_detach(&status,id) {
                        Some(true)=>return Ok(()),
                        Some(false)=>bail!("device could not confirm detach"),
                        None=>{},
                    }
                    continue;
                }
            };
            let terminal = match event {
                PdEvent::State {
                    state: PdState::Detached,
                    command_id,
                    ..
                } if command_id == id => Some(true),
                PdEvent::State {
                    state: PdState::Failed,
                    command_id,
                    ..
                } if command_id == id => Some(false),
                _ => None,
            };
            // A broken output file must not prevent confirming that hardware stopped.
            let _ = recorder.emit(Record::Pd(event));
            match terminal {
                Some(true) => return Ok(()),
                Some(false) => bail!("device could not confirm detach"),
                None => {}
            }
        }
    })
    .await
    .context("timed out waiting for confirmed detach")?
}

fn confirmed_detach(status: &km003c_interface::PdSnapshot, id: u32) -> Option<bool> {
    status
        .last_command
        .filter(|result| result.command_id == id)
        .map(|result| result.released)
}
#[cfg(test)]
mod tests {
    use super::*;
    use km003c_interface::{PdCompletion, PdEndReason, PdSnapshot};
    #[test]
    fn snapshot_confirms_only_matching_command_without_an_event() {
        let mut s = PdSnapshot {
            available: true,
            state: PdState::Detached,
            active_command: None,
            last_session: None,
            last_command: Some(PdCompletion {
                command_id: 12,
                reason: PdEndReason::Stopped,
                released: true,
            }),
        };
        assert_eq!(confirmed_detach(&s, 11), None);
        assert_eq!(confirmed_detach(&s, 12), Some(true));
        s.last_command.as_mut().unwrap().released = false;
        assert_eq!(confirmed_detach(&s, 12), Some(false));
    }
}
