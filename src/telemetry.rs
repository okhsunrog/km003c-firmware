//! Line-oriented CDC diagnostics. Slow/disconnected hosts never block the meter.
//! Separate feature keeps the accepted binary echo image available for USB tests.
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_usb_synopsys_otg::Driver;

pub type Line = heapless::String<512>;
static OUT: Channel<CriticalSectionRawMutex, Line, 8> = Channel::new();
static ONLINE: AtomicBool = AtomicBool::new(false);
static STREAM: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pd-capture")]
#[derive(Clone, Copy)]
enum PdCommand {
    Once(u8),
    Start(u8),
    Stop,
    #[cfg(feature = "pd-tx-test")]
    Attach,
    #[cfg(feature = "pd-tx-test")]
    GetCaps,
    #[cfg(feature = "pd-sink")]
    Sink(u16),
    #[cfg(feature = "pd-bench")]
    Bench,
}
#[cfg(feature = "pd-capture")]
static PD: Channel<CriticalSectionRawMutex, PdCommand, 4> = Channel::new();

pub fn emit(line: Line) {
    if ONLINE.load(Ordering::Relaxed) && OUT.try_send(line).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}
#[cfg(feature = "pd-bench")]
pub(crate) async fn bench_line(line: Line) -> bool {
    embassy_time::with_timeout(embassy_time::Duration::from_millis(250), OUT.send(line))
        .await
        .is_ok()
}
fn reply(message: &str) {
    let mut line = Line::new();
    writeln!(line, "{{\"type\":\"reply\",\"message\":\"{message}\"}}").unwrap();
    emit(line);
}
fn command(text: &str) {
    match text.trim() {
        #[cfg(feature = "pd-bench")]
        "pd bench" => {
            if PD.try_send(PdCommand::Bench).is_err() {
                reply("PD busy");
            }
        }
        "status" => reply("km003c-cdc-v1"),
        "stream on" => {
            STREAM.store(true, Ordering::Relaxed);
            reply("stream on");
        }
        "stream off" => {
            STREAM.store(false, Ordering::Relaxed);
            reply("stream off");
        }
        #[cfg(feature = "pd-capture")]
        "pd 1" | "pd 2" => {
            let input = if text.trim() == "pd 1" { 1 } else { 2 };
            if PD.try_send(PdCommand::Once(input)).is_err() {
                reply("PD busy");
            } else {
                reply("PD capture queued");
            }
        }
        #[cfg(feature = "pd-capture")]
        "pd start 1" | "pd start 2" | "pd start auto" => {
            let input = match text.trim() {
                "pd start 1" => 1,
                "pd start 2" => 2,
                _ => 0,
            };
            if PD.try_send(PdCommand::Start(input)).is_err() {
                reply("PD busy");
            } else {
                reply("PD start queued");
            }
        }
        #[cfg(feature = "pd-capture")]
        "pd stop" => {
            #[cfg(feature = "pd-tx-test")]
            crate::pd_tx::release();
            if PD.try_send(PdCommand::Stop).is_err() {
                reply("PD busy");
            }
        }
        #[cfg(feature = "pd-tx-test")]
        "pd attach 1" | "pd getcaps" => {
            let cmd = if text.trim() == "pd attach 1" {
                PdCommand::Attach
            } else {
                PdCommand::GetCaps
            };
            if PD.try_send(cmd).is_err() {
                reply("PD busy");
            }
        }
        #[cfg(feature = "pd-sink")]
        "pd sink 5" | "pd sink 9" | "pd sink 12" | "pd sink 15" | "pd sink 20" => {
            let volts = text.trim().rsplit(' ').next().unwrap().parse().unwrap();
            if PD.try_send(PdCommand::Sink(volts)).is_err() {
                reply("PD busy");
            }
        }
        // Explicit command, in addition to the physical Exit recovery path.
        "loader" => crate::return_to_loader(0x600d0002),
        "" => (),
        _ => reply("unknown command"),
    }
}

pub async fn receive<'a>(mut receiver: Receiver<'a, Driver<'a>>) {
    loop {
        receiver.wait_connection().await;
        let mut line = heapless::String::<64>::new();
        let mut overflow = false;
        let mut packet = [0; 64];
        while let Ok(n) = receiver.read_packet(&mut packet).await {
            ONLINE.store(true, Ordering::Relaxed);
            for &byte in &packet[..n] {
                if byte == b'\n' {
                    if overflow {
                        reply("command too long or non-ASCII");
                    } else {
                        command(&line);
                    }
                    line.clear();
                    overflow = false;
                } else if byte != b'\r'
                    && (!byte.is_ascii() || line.push(char::from(byte)).is_err())
                {
                    overflow = true;
                }
            }
        }
        ONLINE.store(false, Ordering::Relaxed);
        #[cfg(feature = "pd-tx-test")]
        crate::pd_tx::release();
        OUT.clear();
        #[cfg(feature = "pd-capture")]
        {
            PD.clear();
            let _ = PD.try_send(PdCommand::Stop);
        }
    }
}
pub async fn send<'a>(mut sender: Sender<'a, Driver<'a>>) {
    loop {
        let line = OUT.receive().await;
        // One owner writes whole lines, so ADC and PD records cannot interleave
        // within a line even when either spans several USB packets.
        let result = embassy_time::with_timeout(embassy_time::Duration::from_millis(500), async {
            sender.wait_connection().await;
            for chunk in line.as_bytes().chunks(64) {
                sender.write_packet(chunk).await?;
            }
            if line.len().is_multiple_of(64) {
                sender.write_packet(&[]).await?;
            }
            Ok::<_, embassy_usb::driver::EndpointError>(())
        })
        .await;
        if !matches!(result, Ok(Ok(()))) {
            // A timeout may leave a partial line at the host. A newline before
            // the next record lets an NDJSON reader discard that fragment.
            DROPPED.fetch_add(1, Ordering::Relaxed);
            let _ = embassy_time::with_timeout(
                embassy_time::Duration::from_millis(100),
                sender.write_packet(b"\n"),
            )
            .await;
        }
    }
}
#[embassy_executor::task]
pub async fn measurements() {
    let mut sequence = 0u32;
    loop {
        if STREAM.load(Ordering::Relaxed)
            && ONLINE.load(Ordering::Relaxed)
            && let crate::measurement::State::Sample(sample) = crate::measurement::snapshot()
        {
            let corrected = crate::measurement::corrected(sample);
            let voltage = corrected
                .map(|c| c.voltage_microvolts)
                .unwrap_or((sample.bus_nanovolts() / 1000) as u32);
            let current = corrected
                .map(|c| c.current_microamps)
                .or_else(|| crate::measurement::uncalibrated_microamps(sample));
            let mut line = Line::new();
            write!(
                line,
                "{{\"type\":\"adc\",\"seq\":{sequence},\"ms\":{},\"uv\":{voltage},\"ua\":",
                embassy_time::Instant::now().as_millis()
            )
            .unwrap();
            if let Some(i) = current {
                write!(line, "{i}").unwrap();
            } else {
                line.push_str("null").unwrap();
            }
            writeln!(
                line,
                ",\"mc\":{},\"cal\":{},\"diag\":{},\"dropped\":{}}}",
                sample.temperature_millicelsius(),
                corrected.is_some(),
                sample.diagnostic,
                DROPPED.load(Ordering::Relaxed)
            )
            .unwrap();
            emit(line);
            if let Some(sample) = crate::lines::snapshot() {
                let mut line = Line::new();
                writeln!(line, "{{\"type\":\"lines\",\"seq\":{sequence},\"ms\":{},\"cc1_uv\":{},\"cc2_uv\":{},\"dp_uv\":{},\"dm_uv\":{},\"vdd_uv\":{},\"raw\":{:?},\"cc_input\":{}}}",
                    embassy_time::Instant::now().as_millis(), sample.uv[0], sample.uv[1], sample.uv[2], sample.uv[3], sample.vdd_uv, sample.raw, crate::lines::cc_input().unwrap_or(0)).unwrap();
                emit(line);
            } else {
                reply("line ADC unavailable");
            }
            #[cfg(feature = "orientation")]
            crate::orientation::emit();
            sequence = sequence.wrapping_add(1);
        }
        embassy_time::Timer::after_millis(250).await;
    }
}

#[cfg(feature = "pd-capture")]
static PD_EXECUTOR: embassy_executor::InterruptExecutor =
    embassy_executor::InterruptExecutor::new();
#[cfg(feature = "pd-capture")]
use embassy_hc32::pac::interrupt;
#[cfg(feature = "pd-capture")]
#[interrupt]
fn INT030() {
    unsafe {
        PD_EXECUTOR.on_interrupt();
    }
}
#[cfg(feature = "pd-capture")]
pub fn start_capture_task() {
    // INT030 has no hardware event producer (selector 0x1ff). Its sole
    // producer is Embassy's NVIC pender. Timer/DMA=5, decoder=7, USB=8.
    unsafe {
        (0x4005105cusize.wrapping_add(30 * 4) as *mut u32).write_volatile(0x1ff);
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(interrupt::INT030, 0x70);
    }
    PD_EXECUTOR
        .start(interrupt::INT030)
        .spawn(captures().unwrap());
}
#[cfg(feature = "pd-capture")]
#[derive(Clone, Copy)]
enum PdReport {
    #[cfg(feature = "pd-sink")]
    Tx {
        bytes: [u8; 30],
        len: usize,
        ack_cycles: u32,
        decode_cycles: u32,
        phases: [u32; 4],
    },
    Input {
        input: u8,
    },
    Event {
        event: km003c_drivers::pd::Event,
        burst: u32,
        input: u8,
        end_us: u64,
    },
    Summary {
        burst: u32,
        input: u8,
        count: usize,
        valid: u32,
        errors: u32,
        sync: u32,
    },
    Stats {
        capture: crate::pd_buffered::Stats,
        errors: u32,
        sync: u32,
    },
    FrameError {
        burst: u32,
        code: u8,
        count: usize,
    },
}
#[cfg(feature = "pd-capture")]
static REPORTS: Channel<CriticalSectionRawMutex, PdReport, 32> = Channel::new();
#[cfg(feature = "pd-capture")]
static DECODE_ERRORS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pd-capture")]
static SYNC_ERRORS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pd-capture")]
fn report(value: PdReport) {
    if REPORTS.try_send(value).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}
#[cfg(feature = "pd-sink")]
pub(crate) fn sink_rx(event: km003c_drivers::pd::Event, meta: crate::pd_buffered::Meta) {
    report(PdReport::Event {
        event,
        burst: meta.sequence,
        input: meta.input,
        end_us: meta.end_us,
    });
}
#[cfg(feature = "pd-sink")]
pub(crate) fn sink_tx(data: &[u8]) {
    let mut bytes = [0; 30];
    bytes[..data.len()].copy_from_slice(data);
    let (ack_cycles, decode_cycles) = crate::pd_sink::timing();
    report(PdReport::Tx {
        bytes,
        len: data.len(),
        ack_cycles,
        decode_cycles,
        phases: crate::pd_sink::phases(),
    });
}
#[cfg(feature = "pd-sink")]
pub(crate) fn sink_status(state: &str, mv: u32) {
    let mut line = Line::new();
    writeln!(
        line,
        "{{\"type\":\"pd_sink\",\"state\":\"{state}\",\"target_mv\":{mv}}}"
    )
    .unwrap();
    emit(line);
}
#[cfg(feature = "pd-capture")]
fn pd_stats(r: &crate::pd_buffered::Buffered) {
    report(PdReport::Stats {
        capture: r.stats(),
        errors: DECODE_ERRORS.load(Ordering::Relaxed),
        sync: SYNC_ERRORS.load(Ordering::Relaxed),
    });
}
#[cfg(feature = "pd-capture")]
fn decode_pd(data: &[u16], burst: u32, input: u8, end_us: u64, summary: bool) {
    let mut decoder = km003c_drivers::pd::Decoder::new(20, 5).unwrap();
    let (mut valid, mut errors, mut sync) = (0, 0, 0);
    for &tick in &data[data.len().min(1)..] {
        let in_packet = decoder.in_packet();
        match decoder.push_interval(tick) {
            Ok(Some(event)) => {
                valid += 1;
                report(PdReport::Event {
                    event,
                    burst,
                    input,
                    end_us,
                });
            }
            Err(_) if in_packet => errors += 1,
            Err(_) => sync += 1,
            _ => (),
        }
    }
    if decoder.finish().is_err() {
        errors += 1;
    }
    DECODE_ERRORS.fetch_add(errors, Ordering::Relaxed);
    SYNC_ERRORS.fetch_add(sync, Ordering::Relaxed);
    if summary || errors != 0 || valid == 0 {
        report(PdReport::Summary {
            burst,
            input,
            count: data.len(),
            valid,
            errors,
            sync,
        });
    }
}
/// JSON/hex formatting belongs to the thread executor, after the fast decoder
/// has released its DMA slot. Reports are bounded and never backpressure DMA.
#[cfg(feature = "pd-capture")]
#[embassy_executor::task]
pub async fn pd_reports() {
    loop {
        let record = REPORTS.receive().await;
        let mut line = Line::new();
        match record {
            #[cfg(feature = "pd-sink")]
            PdReport::Tx {
                bytes,
                len,
                ack_cycles,
                decode_cycles,
                phases,
            } => {
                write!(line, "{{\"type\":\"pd_tx\",\"wire_hex\":\"").unwrap();
                for b in &bytes[..len] {
                    write!(line, "{b:02x}").unwrap();
                }
                writeln!(
                    line,
                    "\",\"rx_to_tx_cycles\":{ack_cycles},\"decode_cycles\":{decode_cycles},\"decode_phase_cycles\":{phases:?}}}"
                )
                .unwrap();
            }
            PdReport::Input { input } => {
                writeln!(line, "{{\"type\":\"pd_input\",\"input\":{input}}}").unwrap();
            }
            PdReport::Event {
                event,
                burst,
                input,
                end_us,
            } => match event {
                km003c_drivers::pd::Event::Packet(packet) => {
                    write!(line,"{{\"type\":\"pd_packet\",\"burst\":{burst},\"input\":{input},\"end_us\":{end_us},\"sop\":\"{:?}\",\"header\":{},\"wire_hex\":\"",packet.sop,packet.header()).unwrap();
                    for byte in packet.wire() {
                        write!(line, "{byte:02x}").unwrap();
                    }
                    writeln!(line, "\",\"crc\":{}}}", packet.crc).unwrap();
                }
                reset => {
                    writeln!(
                        line,
                        "{{\"type\":\"pd_reset\",\"burst\":{burst},\"event\":\"{reset:?}\"}}"
                    )
                    .unwrap();
                }
            },
            PdReport::Summary {
                burst,
                input,
                count,
                valid,
                errors,
                sync,
            } => {
                writeln!(line,"{{\"type\":\"pd_summary\",\"burst\":{burst},\"input\":{input},\"count\":{count},\"valid\":{valid},\"decode_errors\":{errors},\"sync_errors\":{sync},\"half_ticks\":20,\"tolerance\":5}}").unwrap();
            }
            PdReport::Stats {
                capture: s,
                errors,
                sync,
            } => {
                writeln!(line,"{{\"type\":\"pd_stats\",\"frames\":{},\"overruns\":{},\"held_overruns\":{},\"noise\":{},\"full\":{},\"dma_errors\":{},\"mismatches\":{},\"stop_faults\":{},\"input_switches\":{},\"switch_discards\":{},\"max_handler_cycles\":{},\"decode_errors\":{errors},\"sync_errors\":{sync}}}",s.frames,s.overruns,s.held_overruns,s.noise,s.full,s.dma_errors,s.mismatches,s.stop_faults,s.input_switches,s.switch_discards,s.max_handler_cycles).unwrap();
            }
            PdReport::FrameError { burst, code, count } => {
                writeln!(line,"{{\"type\":\"pd_frame_error\",\"burst\":{burst},\"code\":{code},\"count\":{count}}}").unwrap();
            }
        }
        emit(line);
    }
}
#[cfg(feature = "pd-capture")]
#[embassy_executor::task]
pub async fn captures() {
    use crate::{
        pd_buffered::Buffered,
        pd_capture::{Input, Receiver},
    };
    use embassy_futures::select::{Either, select};
    let mut receiver = None;
    let mut streaming = false;
    let mut automatic = false;
    let mut input = 1;
    let mut once = [0u16; 860];
    let mut once_sequence = 0u32;
    let mut stats_due = embassy_time::Instant::now();
    loop {
        let command = if streaming {
            let r: &Buffered = receiver.as_ref().unwrap();
            match select(
                PD.receive(),
                embassy_time::with_timeout(
                    embassy_time::Duration::from_millis(if automatic { 20 } else { 1000 }),
                    r.wait_ready(),
                ),
            )
            .await
            {
                Either::First(cmd) => Some(cmd),
                Either::Second(_) => {
                    let r: &mut Buffered = receiver.as_mut().unwrap();
                    let meta = if let Some(frame) = r.frame() {
                        let meta = frame.meta;
                        // Release the DMA slot after a bounded 1720-byte copy,
                        // before BMC/CRC work. The staging array is not a third
                        // DMA target and is also reused by single-burst mode.
                        once[..meta.count].copy_from_slice(frame.samples());
                        Some(meta)
                    } else {
                        None
                    };
                    if let Some(m) = meta {
                        if m.error == 0 {
                            decode_pd(&once[..m.count], m.sequence, m.input, m.end_us, false);
                        } else {
                            report(PdReport::FrameError {
                                burst: m.sequence,
                                code: m.error,
                                count: m.count,
                            });
                        }
                    }
                    if r.faulted() {
                        streaming = false;
                        reply("PD stopped on DMA quiescence fault");
                    }
                    #[cfg(feature = "pd-tx-test")]
                    let may_switch = !crate::pd_tx::attached();
                    #[cfg(not(feature = "pd-tx-test"))]
                    let may_switch = true;
                    if automatic
                        && streaming
                        && may_switch
                        && let Some(next) = crate::lines::cc_input()
                        && next != input
                    {
                        let selected = if next == 1 {
                            Input::Cmp1Inp1
                        } else {
                            Input::Cmp1Inp2
                        };
                        streaming = r.switch_input(selected);
                        if streaming {
                            input = next;
                            report(PdReport::Input { input });
                        } else {
                            reply("PD stopped on input switch fault");
                        }
                    }
                    if !streaming || embassy_time::Instant::now() >= stats_due {
                        pd_stats(r);
                        stats_due =
                            embassy_time::Instant::now() + embassy_time::Duration::from_secs(1);
                    }
                    None
                }
            }
        } else {
            Some(PD.receive().await)
        };
        let Some(command) = command else {
            continue;
        };
        if let PdCommand::Stop = command {
            #[cfg(feature = "pd-tx-test")]
            crate::pd_tx::release();
            if let Some(r) = receiver.as_mut() {
                r.stop();
                pd_stats(r);
            }
            streaming = false;
            reply("PD stopped");
            continue;
        }
        #[cfg(feature = "pd-tx-test")]
        if let PdCommand::GetCaps = command {
            if streaming && input == 1 && !automatic {
                let r = receiver.as_mut().unwrap();
                match crate::pd_tx::get_caps(r).await {
                    Ok(_) => reply("Get_Source_Capabilities wire completed"),
                    Err(e) => {
                        crate::pd_tx::release();
                        reply(e);
                    }
                }
            } else {
                reply("CC1 capture required");
            }
            continue;
        }
        #[cfg(feature = "pd-tx-test")]
        if crate::pd_tx::attached() {
            reply("CC1 lease already active");
            continue;
        }
        // The line-ADC task initializes PA0/PA1 analog mode. Passive commands
        // only observe; the explicit leased probe temporarily delegates PA0.
        let requested = match command {
            PdCommand::Once(i) | PdCommand::Start(i) => i,
            PdCommand::Stop => unreachable!(),
            #[cfg(feature = "pd-tx-test")]
            PdCommand::Attach => 1,
            #[cfg(feature = "pd-tx-test")]
            PdCommand::GetCaps => unreachable!(),
            #[cfg(feature = "pd-sink")]
            PdCommand::Sink(_) => 1,
            #[cfg(feature = "pd-bench")]
            PdCommand::Bench => 1,
        };
        automatic = requested == 0;
        input = if automatic {
            crate::lines::cc_input().unwrap_or(input)
        } else {
            requested
        };
        let selected = if input == 1 {
            Input::Cmp1Inp1
        } else {
            Input::Cmp1Inp2
        };
        if receiver.is_none() {
            match unsafe { Receiver::new(selected, 0x20) } {
                Ok(r) => receiver = Some(Buffered::new(r)),
                Err(e) => {
                    let mut line = Line::new();
                    writeln!(line, "{{\"type\":\"pd_init_error\",\"error\":\"{e:?}\"}}").unwrap();
                    emit(line);
                    return;
                }
            }
        }
        let r = receiver.as_mut().unwrap();
        match command {
            #[cfg(feature = "pd-bench")]
            PdCommand::Bench => {
                streaming = false;
                automatic = false;
                let outcome = select(PD.receive(), crate::pd_bench::run(r)).await;
                crate::pd_tx::release();
                r.stop();
                match outcome {
                    Either::Second(Err(e)) => reply(e),
                    Either::First(_) => reply("PD benchmark stopped"),
                    _ => (),
                }
            }
            #[cfg(feature = "pd-sink")]
            PdCommand::Sink(volts) => {
                streaming = false;
                automatic = false;
                if !r.start(Input::Cmp1Inp1) {
                    reply("PD failed to quiesce DMA");
                    continue;
                }
                if let Err(e) = crate::pd_tx::attach() {
                    reply(e);
                    continue;
                }
                crate::pd_tx::extend_lease();
                sink_status("starting", u32::from(volts) * 1000);
                // Keep the existing recovery/stop command responsive while the
                // stack owns RX/TX. Cancellation releases static DMA/CC controls.
                match select(
                    PD.receive(),
                    embassy_time::with_timeout(
                        embassy_time::Duration::from_secs(20),
                        crate::pd_sink::run(r, volts),
                    ),
                )
                .await
                {
                    Either::First(_) => reply("PD sink stopped by command"),
                    Either::Second(_) => reply("PD sink session finished"),
                }
                crate::pd_tx::release();
                r.stop();
                pd_stats(r);
                sink_status("detached", 0);
            }
            #[cfg(feature = "pd-tx-test")]
            PdCommand::Attach => {
                automatic = false;
                streaming = r.start(Input::Cmp1Inp1);
                if streaming {
                    match crate::pd_tx::attach() {
                        Ok(()) => reply("CC1 attached for 5 seconds"),
                        Err(e) => reply(e),
                    }
                } else {
                    reply("PD failed to quiesce DMA");
                }
            }
            #[cfg(feature = "pd-tx-test")]
            PdCommand::GetCaps => unreachable!(),
            PdCommand::Start(_) => {
                DECODE_ERRORS.store(0, Ordering::Relaxed);
                SYNC_ERRORS.store(0, Ordering::Relaxed);
                streaming = r.start(selected);
                report(PdReport::Input { input });
                reply(if streaming {
                    "PD continuous started"
                } else {
                    "PD failed to quiesce DMA"
                });
            }
            PdCommand::Once(_) => {
                streaming = false;
                r.input(selected);
                embassy_time::Timer::after_micros(20).await;
                once_sequence = once_sequence.wrapping_add(1);
                match r.once(&mut once).await {
                    Ok(count) => {
                        for (index, chunk) in once[..count].chunks(32).enumerate() {
                            let mut line = Line::new();
                            write!(line,"{{\"type\":\"pd_raw\",\"burst\":{once_sequence},\"input\":{input},\"count\":{count},\"offset\":{},\"ticks\":[",index*32).unwrap();
                            for (i, t) in chunk.iter().enumerate() {
                                if i != 0 {
                                    line.push(',').unwrap();
                                }
                                write!(line, "{t}").unwrap();
                            }
                            line.push_str("]}\n").unwrap();
                            if embassy_time::with_timeout(
                                embassy_time::Duration::from_millis(200),
                                OUT.send(line),
                            )
                            .await
                            .is_err()
                            {
                                DROPPED.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                        }
                        decode_pd(
                            &once[..count],
                            once_sequence,
                            input,
                            embassy_time::Instant::now().as_micros(),
                            true,
                        );
                    }
                    Err(e) => {
                        let mut line = Line::new();
                        writeln!(
                            line,
                            "{{\"type\":\"pd_capture_error\",\"error\":\"{e:?}\"}}"
                        )
                        .unwrap();
                        emit(line);
                    }
                }
                let mut line = Line::new();
                writeln!(
                    line,
                    "{{\"type\":\"pd_registers\",\"values\":{:?}}}",
                    r.diagnostics()
                )
                .unwrap();
                emit(line);
            }
            PdCommand::Stop => unreachable!(),
        }
    }
}
