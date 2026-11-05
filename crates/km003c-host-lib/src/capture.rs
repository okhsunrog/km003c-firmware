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

#[derive(Debug)]
enum Input {
    Empty,
    Quit,
    Status,
    Control(PdControl),
}

fn parse_input(line: &str) -> Result<Input> {
    let args: Vec<_> = line.split_whitespace().collect();
    let command = match args.as_slice() {
        [] => return Ok(Input::Empty),
        ["quit"] | ["exit"] => return Ok(Input::Quit),
        ["status"] => return Ok(Input::Status),
        ["stop"] => PdControl::Stop,
        ["profile", volts, current_ma] => PdControl::SetProfile {
            volts: volts.parse()?,
            current_ma: current_ma.parse()?,
        },
        ["sink", volts, current_ma, seconds] => PdControl::Sink {
            volts: volts.parse()?,
            current_ma: current_ma.parse()?,
            seconds: seconds.parse()?,
        },
        ["monitor", "auto"] => PdControl::Monitor { input: None },
        ["monitor", input] => PdControl::Monitor {
            input: Some(input.parse()?),
        },
        _ => bail!(
            "commands: status | profile VOLTS MILLIAMPS | stop | sink VOLTS MILLIAMPS SECONDS | monitor auto/1/2 | quit"
        ),
    };
    crate::rpc::validate_control(command)?;
    Ok(Input::Control(command))
}

/// Use a detached OS thread, not Tokio's uncancellable blocking-stdin worker.
/// A session deadline or Ctrl-C can then return while the terminal is idle.
/// The CLI runs one interactive session per process; process exit ends the reader.
fn stdin_lines() -> tokio::sync::mpsc::Receiver<std::io::Result<String>> {
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let failed = line.is_err();
            if tx.blocking_send(line).is_err() || failed {
                break;
            }
        }
    });
    rx
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
    run_interactive(client, plan, output, false).await
}

pub async fn run_interactive(
    client: &Client,
    plan: &Maneuver,
    output: Option<&Path>,
    interactive: bool,
) -> Result<()> {
    plan.validate()?;
    let mut recorder = Recorder::new(output)?;
    recorder.emit(Record::Maneuver(plan))?;
    let mut measurements = client.rpc.subscribe_exclusive::<Measurements>(32).await?;
    let mut pd = client.rpc.subscribe_exclusive::<PdEvents>(64).await?;
    let mut logs = client.rpc.subscribe_exclusive::<LoggingTopic>(32).await?;
    let mut input = interactive.then(stdin_lines);
    let mut input_open = interactive;
    let mut interactive_index = plan.steps.len();
    let result: Result<()> = async {
        client.stream_interval(plan.measurement_interval_ms).await?;
        client.stream(true).await?;
        let start = Instant::now();
        let end = start + Duration::from_millis(plan.duration_ms);
        let mut index = 0;
        let mut status_poll = tokio::time::interval(Duration::from_millis(250));
        status_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut previous_status = None;
        loop {
            let due = plan.steps.get(index).map_or(end, |s| {
                start + Duration::from_millis(s.at_ms)
            });
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                _ = tokio::time::sleep_until(end) => break,
                _ = status_poll.tick() => {
                    let status = client.pd_status().await?;
                    if previous_status != Some(status) {
                        recorder.emit(Record::PdSnapshot(status))?;
                        if interactive {
                            eprintln!("PD {:?}; contract={:?}; pending={:?}; last_profile={:?}",
                                status.state, status.contract, status.pending_profile, status.last_profile);
                        }
                        previous_status = Some(status);
                    }
                }
                _ = tokio::time::sleep_until(due), if index < plan.steps.len() => {
                    let Step { command, .. } = plan.steps[index];
                    recorder.emit(Record::Command { index, command })?;
                    let device_command = client.control(command).await?;
                    recorder.emit(Record::Accepted { index, device_command })?;
                    if matches!(command, PdControl::Stop) {
                        wait_detached(client, &mut pd, device_command, &mut recorder).await?;
                    }
                    index += 1;
                }
                line = async { input.as_mut().unwrap().recv().await }, if input_open => {
                    let Some(line) = line else {
                        input_open = false;
                        continue;
                    };
                    match parse_input(&line?) {
                        Ok(Input::Quit) => break,
                        Ok(Input::Status) => {
                            let status = client.pd_status().await?;
                            eprintln!("{status:#?}");
                            recorder.emit(Record::PdSnapshot(status))?;
                        }
                        Ok(Input::Empty) => {}
                        Ok(Input::Control(command)) => {
                            let index = interactive_index;
                            interactive_index += 1;
                            recorder.emit(Record::Command { index, command })?;
                            match client.control(command).await {
                                Ok(device_command) => {
                                    recorder.emit(Record::Accepted { index, device_command })?;
                                    eprintln!("Accepted command {device_command}; awaiting device status");
                                    if matches!(command, PdControl::Stop) {
                                        wait_detached(client, &mut pd, device_command, &mut recorder).await?;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("{e:#}");
                                    recorder.emit(Record::Error(e.to_string()))?;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("{e:#}");
                            recorder.emit(Record::Error(e.to_string()))?;
                        }
                    }
                }
                msg = measurements.recv() => {
                    recorder.emit(Record::Measurement(msg.context("measurement stream closed")?))?;
                }
                msg = pd.recv() => recorder.emit(Record::Pd(msg.context("PD stream closed")?))?,
                msg = logs.recv() => recorder.emit(Record::Log(msg.context("logging stream closed")?))?,
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
    // Stop completion can arrive over RPC before its queued terminal Stats
    // topic. Keep reading briefly so normal cleanup does not discard that tail.
    let drain_until = Instant::now() + Duration::from_millis(100);
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(drain_until) => break,
            msg = pd.recv() => match msg {
                Some(event) => recorder.emit(Record::Pd(event))?,
                None => break,
            },
            msg = measurements.recv() => match msg {
                Some(sample) => recorder.emit(Record::Measurement(sample))?,
                None => break,
            },
            msg = logs.recv() => match msg {
                Some(line) => recorder.emit(Record::Log(line))?,
                None => break,
            },
        }
    }
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
        let mut events_open = true;
        let mut status_poll = tokio::time::interval(Duration::from_millis(100));
        status_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let event = tokio::select! {
                msg = pd.recv(), if events_open => match msg {
                    Some(event) => event,
                    None => {
                        events_open = false;
                        continue;
                    }
                },
                _ = status_poll.tick() => {
                    let status = client.pd_status().await?;
                    match confirmed_detach(&status, id) {
                        Some(true) => return Ok(()),
                        Some(false) => bail!("device could not confirm detach"),
                        None => {}
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
    fn interactive_commands_use_explicit_units_and_reject_invalid_profiles() {
        assert!(matches!(
            parse_input("profile 15 3000").unwrap(),
            Input::Control(PdControl::SetProfile {
                volts: 15,
                current_ma: 3000
            })
        ));
        assert!(matches!(
            parse_input("monitor auto").unwrap(),
            Input::Control(PdControl::Monitor { input: None })
        ));
        assert!(parse_input("profile 28 3000").is_err());
        assert!(parse_input("profile 15 5000").is_err());
        assert!(parse_input("sink 20 100 601").is_err());
        assert!(parse_input("profile 15").is_err());
        assert!(parse_input("monitor 3").is_err());
    }

    #[test]
    fn snapshot_confirms_only_matching_command_without_an_event() {
        let mut s = PdSnapshot {
            available: true,
            state: PdState::Detached,
            active_command: None,
            last_session: None,
            contract: None,
            selected_profile: None,
            pending_profile: None,
            last_profile: None,
            source_profiles: [None; 7],
            event_queue_drops: 0,
            event_publish_failures: 0,
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
