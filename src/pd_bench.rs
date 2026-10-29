//! DWT comparison with outputs released and DMA stopped. Run all implementations
//! in one image, on identical cached data; never benchmark inside a GoodCRC path.
use crate::{
    pd_buffered::Buffered,
    pd_fast,
    telemetry::{self, Line},
};
use core::{fmt::Write, hint::black_box};
use km003c_drivers::pd::{
    self, Event, Sop,
    tx::{self, Frame, Periods},
};
const SAMPLES: usize = 128;
#[inline(always)]
fn cycles() -> u32 {
    unsafe { (0xe0001004 as *const u32).read_volatile() }
}
async fn send(line: Line) -> Result<(), &'static str> {
    if telemetry::bench_line(line).await {
        Ok(())
    } else {
        Err("benchmark host stalled")
    }
}
fn allowed(name: &str) -> bool {
    !name.ends_with("_word") || pd_fast::unaligned_enabled()
}

async fn fixture(name: &str, data: &[u16]) -> Result<(), &'static str> {
    for (i, chunk) in data.chunks(32).enumerate() {
        let mut line = Line::new();
        writeln!(line, "{{\"type\":\"pd_bench_fixture\",\"name\":\"{name}\",\"count\":{},\"offset\":{},\"ticks\":{chunk:?}}}", data.len(), i*32).unwrap();
        send(line).await?;
    }
    let expected = pd::burst::decode(data, 20, 5).map_err(|_| "fixture rejected")?;
    for &(candidate, decode) in pd_fast::CANDIDATES {
        if !allowed(candidate) {
            continue;
        }
        for _ in 0..8 {
            if black_box(decode(black_box(data))) != Ok(expected) {
                return Err("candidate fixture mismatch");
            }
        }
        let mut samples = [0u32; SAMPLES];
        // IRQs stay enabled, as in production. Quantiles expose occasional IRQ
        // service; min approximates the uncontended warm-code cost.
        for value in &mut samples {
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let begin = cycles();
            let result = black_box(decode(black_box(data)));
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            *value = cycles().wrapping_sub(begin);
            if result != Ok(expected) {
                return Err("candidate sample mismatch");
            }
        }
        let raw = samples;
        samples.sort_unstable();
        let mut line = Line::new();
        writeln!(line,"{{\"type\":\"pd_bench\",\"fixture\":\"{name}\",\"candidate\":\"{candidate}\",\"samples\":{SAMPLES},\"min\":{},\"p50\":{},\"p95\":{},\"max\":{},\"hz\":192000000}}",samples[0],samples[SAMPLES/2],samples[SAMPLES*95/100],samples[SAMPLES-1]).unwrap();
        send(line).await?;
        for (i, chunk) in raw.chunks(32).enumerate() {
            let mut line = Line::new();
            writeln!(line,"{{\"type\":\"pd_bench_samples\",\"fixture\":\"{name}\",\"candidate\":\"{candidate}\",\"offset\":{},\"cycles\":{chunk:?}}}",i*32).unwrap();
            send(line).await?;
        }
        // Give USB/UI/ADC thread work time between bounded, timed batches.
        embassy_time::Timer::after_millis(10).await;
    }
    Ok(())
}
async fn gate() -> Result<u32, &'static str> {
    let mut count = 0;
    let mut wire = [0u8; 30];
    let mut reload = [0u16; 860];
    let mut jitter = [0u16; 860];
    for sop in [
        Sop::Sop,
        Sop::Prime,
        Sop::DoublePrime,
        Sop::PrimeDebug,
        Sop::DoublePrimeDebug,
    ] {
        for ndo in 0..8 {
            for fill in [0u8, 255, 85, 170] {
                wire.fill(fill);
                wire[0] = 0x81;
                wire[1] = (ndo as u8) << 4;
                let n = tx::encode(
                    Frame::Packet {
                        sop,
                        wire: &wire[..2 + 4 * ndo],
                    },
                    Periods::NATIVE_207,
                    &mut reload,
                )
                .unwrap();
                for k in 0..n {
                    reload[k] += 1;
                    jitter[k] = (i32::from(reload[k]) + [-4, 0, 4][k % 3]) as u16;
                }
                for data in [&reload[..n], &jitter[..n]] {
                    let expected = pd::burst::decode(data, 20, 5).unwrap();
                    for &(name, decode) in pd_fast::CANDIDATES {
                        if allowed(name) && decode(data) != Ok(expected) {
                            return Err("candidate vector mismatch");
                        }
                    }
                    count += 1;
                }
                embassy_time::Timer::after_millis(1).await;
            }
        }
    }
    for frame in [Frame::HardReset, Frame::CableReset] {
        let n = tx::encode(frame, Periods::NATIVE_207, &mut reload).unwrap();
        for x in &mut reload[..n] {
            *x += 1;
        }
        let expected = pd::burst::decode(&reload[..n], 20, 5).unwrap();
        for &(name, decode) in pd_fast::CANDIDATES {
            if allowed(name) && decode(&reload[..n]) != Ok(expected) {
                return Err("candidate reset mismatch");
            }
        }
        count += 1;
    }
    Ok(count)
}
pub async fn run(rx: &mut Buffered) -> Result<(), &'static str> {
    let mut live = [0u16; 860];
    let selected = crate::pd_tx::prepare().await?;
    if !rx.start(selected) {
        return Err("benchmark RX DMA did not stop");
    }
    crate::pd_tx::attach(selected).await?;
    let capture = embassy_time::with_timeout(embassy_time::Duration::from_secs(2), async {
        loop {
            let Some(f) = rx.frame() else {
                rx.wait_ready().await;
                continue;
            };
            let ticks = &f.samples()[f.meta.count.min(1)..];
            if let Ok(Some(Event::Packet(p))) = pd::burst::decode(ticks, 20, 5)
                && p.sop == Sop::Sop
                && p.wire().len() > 2
                && p.header() & 31 == 1
            {
                live[..ticks.len()].copy_from_slice(ticks);
                break ticks.len();
            }
        }
    })
    .await;
    crate::pd_tx::release_and_wait().await?;
    if !rx.stop() {
        return Err("benchmark RX DMA did not stop");
    }
    let live_len = capture.map_err(|_| "no live Source Capabilities")?;
    let vectors = gate().await?;
    let mut line = Line::new();
    writeln!(
        line,
        "{{\"type\":\"pd_bench_gate\",\"cases\":{vectors},\"candidates\":{},\"unaligned\":{}}}",
        pd_fast::CANDIDATES.len(),
        pd_fast::unaligned_enabled()
    )
    .unwrap();
    send(line).await?;
    fixture("source_caps", &live[..live_len]).await?;
    let input = include_str!("../crates/km003c-drivers/tests/data/pixel8pro-request.intervals");
    let mut n = 0;
    for s in input.split_whitespace() {
        live[n] = s.parse().unwrap();
        n += 1;
    }
    fixture("pixel_request", &live[1..n]).await?;
    let mut wire = [255u8; 30];
    wire[0] = 0x81;
    wire[1] = 0x70;
    let n = tx::encode(
        Frame::Packet {
            sop: Sop::Sop,
            wire: &wire,
        },
        Periods::NATIVE_207,
        &mut live,
    )
    .unwrap();
    for x in &mut live[..n] {
        *x += 1;
    }
    fixture("max_packet", &live[..n]).await?;
    let mut line = Line::new();
    writeln!(line, "{{\"type\":\"pd_bench_done\",\"cases\":{vectors}}}").unwrap();
    send(line).await
}

/// Alarm acceptance without CC attachment. Read the physical counter directly
/// as a reference: DWT is reserved for active CPU work and is not a sleep clock.
pub async fn clock() -> Result<(), &'static str> {
    use embassy_time::{Duration, Instant, Timer};
    let (_, hz, _, _) = embassy_hc32::time_driver::diagnostics();
    // Exercise now() across a physical wrap while the calling executor is
    // running. The handler is lower priority; now() must account pending wrap.
    let begin = cycles();
    let mut previous = Instant::now();
    while cycles().wrapping_sub(begin) < 38_400_000 {
        let next = Instant::now();
        if next < previous || (next - previous).as_ticks() > 100 {
            return Err("clock discontinuity across hardware wrap");
        }
        previous = next;
    }
    for (ms, padded) in [(1, false), (1, true), (5, true), (20, true)] {
        let mut samples = [0u16; 64];
        for (i, value) in samples.iter_mut().enumerate() {
            // Sweep start phase through one logical tick, exposing rounding
            // rather than measuring only phase-locked back-to-back wakeups.
            cortex_m::asm::delay(((i * 193) % 6000) as u32);
            let start = unsafe { (0x40015000 as *const u16).read_volatile() };
            Timer::after(Duration::from_millis(ms) + Duration::from_ticks(u64::from(padded))).await;
            *value = unsafe { (0x40015000 as *const u16).read_volatile() }.wrapping_sub(start);
        }
        let mut sorted = samples;
        sorted.sort_unstable();
        let mut line = Line::new();
        writeln!(line,"{{\"type\":\"time_bench\",\"ms\":{ms},\"padded\":{padded},\"tick_hz\":{},\"counter_hz\":{hz},\"min_counts\":{},\"p50_counts\":{},\"max_counts\":{}}}",embassy_time::TICK_HZ,sorted[0],sorted[32],sorted[63]).unwrap();
        send(line).await?;
        for (i, chunk) in samples.chunks(32).enumerate() {
            let mut line = Line::new();
            writeln!(line,"{{\"type\":\"time_bench_samples\",\"ms\":{ms},\"padded\":{padded},\"offset\":{},\"counts\":{chunk:?}}}",i*32).unwrap();
            send(line).await?;
        }
        if padded && u64::from(sorted[0]) * 1_000_000 < u64::from(hz) * ms * 1000 {
            return Err("timer expired early");
        }
    }
    let before = embassy_hc32::time_driver::diagnostics();
    let profile_before = crate::time_stats::snapshot();
    let start = Instant::now();
    Timer::after_millis(1100).await;
    let elapsed = Instant::now() - start;
    let after = embassy_hc32::time_driver::diagnostics();
    let profile_after = crate::time_stats::snapshot();
    for (i, (a, b)) in profile_before.into_iter().zip(profile_after).enumerate() {
        let mut line = Line::new();
        writeln!(line,"{{\"type\":\"time_irq_cost\",\"source\":{i},\"calls\":{},\"cycles\":{},\"max_cycles\":{},\"window_ticks\":{}}}",b[0].wrapping_sub(a[0]),b[1].wrapping_sub(a[1]),b[2],elapsed.as_ticks()).unwrap();
        send(line).await?;
    }
    let systick = unsafe { (0xe000e010 as *const u32).read_volatile() } & 3;
    let mut line = Line::new();
    writeln!(line,"{{\"type\":\"time_bench_done\",\"elapsed_ticks\":{},\"irq_delta\":{},\"wrap_delta\":{},\"systick_enable\":{systick}}}",elapsed.as_ticks(),after.2.wrapping_sub(before.2),after.3.wrapping_sub(before.3)).unwrap();
    send(line).await
}
