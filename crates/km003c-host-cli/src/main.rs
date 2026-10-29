//! CLI orchestration; transport, recording and update policy live in host-lib.
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use km003c_host_lib::{
    DeviceSelector, capture,
    interface::PdControl,
    maintenance,
    maneuver::{Maneuver, Step},
    package,
    rpc::Client,
};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "km003c",
    about = "KM003C custom firmware control, capture and maintenance"
)]
struct Args {
    /// Select by USB serial; maintenance retains the physical port across resets.
    #[arg(long, global = true)]
    serial: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Status,
    /// Current PD owner and last command/session results.
    PdStatus,
    Capture {
        #[arg(long, default_value_t = 6)]
        seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Monitor {
        #[arg(default_value = "auto")]
        input: String,
        #[arg(long, default_value_t = 15)]
        seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Sink {
        volts: u16,
        #[arg(long, default_value_t = 6)]
        seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Stop,
    Loader,
    /// Run a validated JSON timeline and record its commands, events and final Stop.
    Maneuver {
        plan: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Build a validated .mencrypt package offline from a linked raw binary.
    Package {
        binary: PathBuf,
        output: PathBuf,
    },
    /// Save the current application from DFU, verifying its committed CRC.
    Backup {
        output: PathBuf,
    },
    /// Write from DFU/RPC; the evidence directory must be new.
    Flash {
        image: PathBuf,
        #[arg(long)]
        evidence: PathBuf,
    },
    Verify {
        image: PathBuf,
    },
    /// Start an installed application from native DFU without writing flash.
    Start,
    /// Read retained panic/trace RAM while in DFU.
    Diagnostics {
        output: PathBuf,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let selector = args
        .serial
        .map_or(DeviceSelector::First, DeviceSelector::SerialNumber);
    match args.command {
        Command::Package { binary, output } => {
            let data = package::package(&std::fs::read(&binary)?)?;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            file.write_all(&data)?;
            file.sync_all()?;
            println!("Packaged {} bytes into {}", data.len(), output.display());
        }
        Command::Backup { output } => {
            maintenance::backup(selector, &output).await?;
            println!("Verified backup: {}", output.display());
        }
        Command::Flash { image, evidence } => {
            let image = km003c_host_lib::maintenance::parse_image(&std::fs::read(image)?)?;
            maintenance::flash(selector, &image, &evidence).await?;
            println!(
                "Verified application and unchanged loader; device remains in DFU. Run start separately."
            );
        }
        Command::Verify { image } => {
            let image = maintenance::parse_image(&std::fs::read(image)?)?;
            maintenance::verify(selector, &image).await?;
            println!("Application and loader verified");
        }
        Command::Start => {
            maintenance::start(selector).await?;
            println!("Application started without flash writes");
        }
        Command::Diagnostics { output } => maintenance::diagnostics(selector, &output).await?,
        Command::Maneuver { plan, output } => {
            let plan: Maneuver = serde_json_from_path(&plan)?;
            plan.validate()?;
            let client = Client::connect(selector).await?;
            capture::run(&client, &plan, Some(&output)).await?;
        }
        command => {
            // Validate arguments before connecting or enabling any stream/output.
            let plan = match &command {
                Command::Capture { seconds, .. } => Some(plan(*seconds, Vec::new())?),
                Command::Monitor { input, seconds, .. } => Some(plan(
                    *seconds,
                    vec![PdControl::Monitor {
                        input: if input == "auto" {
                            None
                        } else {
                            Some(input.parse().context("input must be auto, 1 or 2")?)
                        },
                    }],
                )?),
                Command::Sink { volts, seconds, .. } => {
                    ensure!(*seconds <= 590, "sink capture exceeds maximum lease");
                    Some(plan(
                        *seconds,
                        vec![PdControl::Sink {
                            volts: *volts,
                            seconds: u16::try_from(*seconds + 5)?,
                        }],
                    )?)
                }
                Command::Stop => Some(plan(1, vec![PdControl::Stop])?),
                _ => None,
            };
            if let Some(plan) = &plan {
                plan.validate()?;
            }
            let client = Client::connect(selector).await?;
            match command {
                Command::Status => println!("{:?}", client.status().await?),
                Command::PdStatus => println!("{:?}", client.pd_status().await?),
                Command::Loader => {
                    client.loader().await?;
                    println!("Native DFU confirmed on the selected port");
                }
                Command::Capture { output, .. }
                | Command::Monitor { output, .. }
                | Command::Sink { output, .. } => {
                    capture::run(&client, plan.as_ref().unwrap(), output.as_deref()).await?
                }
                Command::Stop => capture::run(&client, plan.as_ref().unwrap(), None).await?,
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}
fn plan(seconds: u64, commands: Vec<PdControl>) -> Result<Maneuver> {
    ensure!(
        (1..=600).contains(&seconds),
        "capture duration must be 1..600 seconds"
    );
    Ok(Maneuver {
        name: "cli".into(),
        duration_ms: seconds * 1000,
        steps: commands
            .into_iter()
            .map(|command| Step { at_ms: 0, command })
            .collect(),
    })
}
fn serde_json_from_path(path: &std::path::Path) -> Result<Maneuver> {
    km003c_host_lib::maneuver::load(path)
}
