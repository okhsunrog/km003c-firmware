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
static PD: Channel<CriticalSectionRawMutex, u8, 1> = Channel::new();

pub fn emit(line: Line) {
    if ONLINE.load(Ordering::Relaxed) && OUT.try_send(line).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}
fn reply(message: &str) {
    let mut line = Line::new();
    writeln!(line, "{{\"type\":\"reply\",\"message\":\"{message}\"}}").unwrap();
    emit(line);
}
fn command(text: &str) {
    match text.trim() {
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
            if PD.try_send(input).is_err() {
                reply("PD busy");
            } else {
                reply("PD capture queued");
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
        OUT.clear();
        #[cfg(feature = "pd-capture")]
        PD.clear();
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
            #[cfg(feature = "orientation")]
            crate::orientation::emit();
            sequence = sequence.wrapping_add(1);
        }
        embassy_time::Timer::after_millis(250).await;
    }
}

#[cfg(feature = "pd-capture")]
#[embassy_executor::task]
pub async fn captures() {
    use crate::pd_capture::{Input, Receiver};
    // PA0/PA1 are CMP1_INP1/2 (datasheet pin table). AnalogPin disables
    // digital outputs/pull-ups. This does not assert a CC1/CC2 connector mapping.
    let mut receiver = None;
    let mut pins = None;
    let mut buffer = [0u16; 860];
    let mut burst = 0u32;
    loop {
        let input = PD.receive().await;
        if pins.is_none() {
            pins = Some(unsafe {
                [
                    embassy_hc32::gpio::AnalogPin::new(embassy_hc32::gpio::Port::A, 0),
                    embassy_hc32::gpio::AnalogPin::new(embassy_hc32::gpio::Port::A, 1),
                ]
            });
        }
        let selected = if input == 1 {
            Input::Cmp1Inp1
        } else {
            Input::Cmp1Inp2
        };
        if receiver.is_none() {
            // Split DMA1 exactly once. No other task owns its resources.
            match unsafe { Receiver::new(selected, 0x20) } {
                Ok(r) => receiver = Some(r),
                Err(e) => {
                    let mut line = Line::new();
                    writeln!(line, "{{\"type\":\"pd_init_error\",\"error\":\"{e:?}\"}}").unwrap();
                    emit(line);
                    return;
                }
            }
        }
        let r = receiver.as_mut().unwrap();
        r.select_input(selected);
        embassy_time::Timer::after_micros(20).await;
        burst = burst.wrapping_add(1);
        match r
            .capture(&mut buffer, embassy_time::Duration::from_secs(15))
            .await
        {
            Ok(count) => {
                // Bound host backpressure; rearm is deliberately command-driven.
                // Raw chunks preserve every interval, not only decoder successes.
                for (index, chunk) in buffer[..count].chunks(32).enumerate() {
                    let mut line = Line::new();
                    write!(line,"{{\"type\":\"pd_raw\",\"burst\":{burst},\"input\":{input},\"count\":{count},\"offset\":{},\"ticks\":[",index*32).unwrap();
                    for (i, tick) in chunk.iter().enumerate() {
                        if i != 0 {
                            line.push(',').unwrap();
                        }
                        write!(line, "{tick}").unwrap();
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
                // Measured edge groups are 15/19/23 and 35/39/43 ticks.
                // Five ticks accommodate the comparator/filter asymmetry while
                // keeping the short and long timing windows disjoint.
                let mut decoder = km003c_drivers::pd::Decoder::new(20, 5).unwrap();
                let (mut valid, mut errors) = (0u32, 0u32);
                // The first capture starts a cleared timer, so it has no
                // preceding edge. Keep it in raw output, skip it in decoding.
                for &tick in &buffer[count.min(1)..count] {
                    match decoder.push_interval(tick) {
                        Ok(Some(event)) => {
                            valid += 1;
                            let mut line = Line::new();
                            match event {
                                km003c_drivers::pd::Event::Packet(packet) => {
                                    write!(line,"{{\"type\":\"pd_packet\",\"burst\":{burst},\"input\":{input},\"ms\":{},\"sop\":\"{:?}\",\"header\":{},\"wire_hex\":\"",embassy_time::Instant::now().as_millis(),packet.sop,packet.header()).unwrap();
                                    for byte in packet.wire() {
                                        write!(line, "{byte:02x}").unwrap();
                                    }
                                    writeln!(line, "\",\"crc\":{}}}", packet.crc).unwrap();
                                }
                                reset => {
                                    writeln!(line,"{{\"type\":\"pd_reset\",\"burst\":{burst},\"event\":\"{reset:?}\"}}").unwrap();
                                }
                            }
                            emit(line);
                        }
                        Err(_) => errors += 1,
                        _ => (),
                    }
                }
                if decoder.finish().is_err() {
                    errors += 1;
                }
                let mut line = Line::new();
                writeln!(line,"{{\"type\":\"pd_summary\",\"burst\":{burst},\"input\":{input},\"count\":{count},\"valid\":{valid},\"decode_errors\":{errors},\"half_ticks\":20,\"tolerance\":5}}").unwrap();
                emit(line);
            }
            Err(e) => {
                let mut line = Line::new();
                writeln!(line,"{{\"type\":\"pd_capture_error\",\"burst\":{burst},\"input\":{input},\"error\":\"{e:?}\"}}").unwrap();
                emit(line);
            }
        }
        let mut line = Line::new();
        writeln!(
            line,
            "{{\"type\":\"pd_registers\",\"burst\":{burst},\"values\":{:?}}}",
            r.diagnostics()
        )
        .unwrap();
        emit(line);
    }
}
