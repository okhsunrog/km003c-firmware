//! Slint's software renderer owns only a small tile; panel GRAM keeps the frame.
use alloc::{boxed::Box, rc::Rc};
use core::{fmt::Write, ops::Range};
use embassy_hc32::gpio::{InputPin, Port};
use embedded_graphics::{
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::*,
    primitives::Rectangle,
};
use slint::{
    ComponentHandle,
    platform::{
        Key, Platform, PlatformError, WindowAdapter, WindowEvent,
        software_renderer::{
            LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
        },
    },
};

const WIDTH: usize = 240;
const TILE_LINES: usize = 8;
type Value = heapless::String<24>;

struct BoardPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for BoardPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_micros(embassy_time::Instant::now().as_micros())
    }
}

// Batch equal-width adjacent lines into one QSPI transaction. The panel owns
// the complete image; no full framebuffer or external flash mapping is needed.
struct Lines<'a> {
    display: &'a mut crate::display::Display,
    pixels: [Rgb565; WIDTH * TILE_LINES],
    line: [Rgb565Pixel; WIDTH],
    x: usize,
    y: usize,
    width: usize,
    rows: usize,
}
impl<'a> Lines<'a> {
    fn new(display: &'a mut crate::display::Display) -> Self {
        Self {
            display,
            pixels: [Rgb565::BLACK; WIDTH * TILE_LINES],
            line: [Rgb565Pixel(0); WIDTH],
            x: 0,
            y: 0,
            width: 0,
            rows: 0,
        }
    }
    fn flush(&mut self) {
        if self.rows == 0 {
            return;
        }
        let area = Rectangle::new(
            Point::new(self.x as i32, self.y as i32),
            Size::new(self.width as u32, self.rows as u32),
        );
        self.display
            .blit(area, &self.pixels[..self.width * self.rows]);
        self.rows = 0;
    }
}
impl LineBufferProvider for &mut Lines<'_> {
    type TargetPixel = Rgb565Pixel;
    fn process_line(
        &mut self,
        line: usize,
        range: Range<usize>,
        render: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        if range.is_empty() {
            return;
        }
        let width = range.len();
        if self.rows != 0
            && (self.x != range.start
                || self.width != width
                || self.y + self.rows != line
                || self.rows == TILE_LINES)
        {
            self.flush();
        }
        if self.rows == 0 {
            self.x = range.start;
            self.y = line;
            self.width = width;
        }
        render(&mut self.line[..width]);
        let start = self.rows * width;
        for (dst, src) in self.pixels[start..start + width]
            .iter_mut()
            .zip(&self.line[..width])
        {
            *dst = Rgb565::from(RawU16::new(src.0));
        }
        self.rows += 1;
    }
}
fn label(text: &str) -> Value {
    Value::try_from(text).unwrap()
}
fn decimal(value: i64, places: u32, unit: &str) -> Value {
    let mut out = Value::new();
    let divisor = 10u64.pow(places);
    let magnitude = value.unsigned_abs();
    write!(
        out,
        "{}{}.{:0width$} {}",
        if value < 0 { "-" } else { "" },
        magnitude / divisor,
        magnitude % divisor,
        unit,
        width = places as usize
    )
    .unwrap();
    out
}
fn key(window: &slint::Window, key: Key, down: bool) {
    let event = if down {
        WindowEvent::KeyPressed { text: key.into() }
    } else {
        WindowEvent::KeyReleased { text: key.into() }
    };
    window.dispatch_event(event);
}

#[embassy_executor::task]
pub async fn task(aw: crate::aw9523::Aw9523) {
    aw.lcd_reset().await.unwrap();
    aw.backlight(0).await.unwrap();
    let mut display = unsafe { crate::display::Display::new() };
    let (id, block) = display.factory_settings();
    crate::measurement::factory_settings(id, &block);
    let mut sensor_bus = aw.sensor_bus();
    let mut orientation = crate::orientation::probe(&mut sensor_bus).await;
    let mut rotation = if orientation.is_some() { 3 } else { 0 };
    display.set_rotation(rotation);
    display.init().await;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(BoardPlatform(window.clone()))).unwrap();
    let ui = crate::MeterWindow::new().unwrap();
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, WIDTH as u32));
    ui.show().unwrap();
    let buttons = unsafe {
        [
            InputPin::new_pull_up(Port::B, 7),
            InputPin::new_pull_up(Port::B, 11),
            InputPin::new_pull_up(Port::B, 8),
            InputPin::new_pull_up(Port::B, 9),
        ]
    };
    let keys = [Key::Escape, Key::Return, Key::LeftArrow, Key::RightArrow];
    let (mut previous, mut stable, mut repeats) = (0u8, 0u8, 0u8);
    let (mut lit, mut held_exit) = (false, 0u16);
    let mut force_repaint = true;
    let mut lines = Lines::new(&mut display);
    macro_rules! set_changed {
        ($get:ident, $set:ident, $value:expr) => {{
            let value = $value;
            if ui.$get().as_str() != value.as_str() {
                ui.$set(value.as_str().into());
            }
        }};
    }
    log::info!(
        "Slint meter ready; hold EXIT for loader; heap free {}",
        crate::HEAP.free()
    );
    loop {
        if let Some(sensor) = orientation.as_mut() {
            let next = sensor.poll(&mut sensor_bus).await;
            if next != rotation {
                rotation = next;
                lines.display.set_rotation(rotation);
                // Repaint every pixel in the new orientation, then return to dirty regions.
                force_repaint = true;
                window.request_redraw();
            }
        }
        let mut raw = 0;
        for (i, button) in buttons.iter().enumerate() {
            if button.is_low() {
                raw |= 1 << i;
            }
        }
        if raw == previous {
            repeats = (repeats + 1).min(3);
        } else {
            previous = raw;
            repeats = 0;
        }
        if repeats == 3 && raw != stable {
            for (i, &button_key) in keys.iter().enumerate() {
                let mask = 1 << i;
                if (raw ^ stable) & mask != 0 {
                    key(ui.window(), button_key, raw & mask != 0);
                }
            }
            stable = raw;
        }
        let values = match crate::measurement::snapshot() {
            crate::measurement::State::Sample(sample) => {
                let calibrated = crate::measurement::corrected(sample);
                let voltage = calibrated
                    .map(|v| v.voltage_microvolts)
                    .unwrap_or((sample.bus_nanovolts() / 1000) as u32);
                let current = calibrated
                    .map(|v| v.current_microamps)
                    .or_else(|| crate::measurement::uncalibrated_microamps(sample));
                [
                    decimal(i64::from(voltage) / 1000, 3, "V"),
                    current
                        .map(|i| decimal(i64::from(i) / 1000, 3, "A"))
                        .unwrap_or_else(|| label("-- A")),
                    current
                        .map(|i| {
                            decimal(i64::from(voltage) * i64::from(i) / 10_000_000_000, 2, "W")
                        })
                        .unwrap_or_else(|| label("-- W")),
                    decimal(i64::from(sample.temperature_millicelsius()) / 100, 1, "C"),
                    label(if calibrated.is_some() { "CAL" } else { "RAW" }),
                ]
            }
            crate::measurement::State::Probing => [
                label("STARTING"),
                label("-- A"),
                label("-- W"),
                label("-- C"),
                label("WAIT"),
            ],
            _ => [
                label("ADC ERROR"),
                label("-- A"),
                label("-- W"),
                label("-- C"),
                label("ERROR"),
            ],
        };
        set_changed!(get_voltage, set_voltage, values[0].clone());
        set_changed!(get_current, set_current, values[1].clone());
        set_changed!(get_power, set_power, values[2].clone());
        set_changed!(get_temperature, set_temperature, values[3].clone());
        set_changed!(get_calibration, set_calibration, values[4].clone());
        let sample = crate::lines::snapshot();
        let line_value = |i: usize| {
            sample
                .map(|s| decimal(i64::from(s.uv[i] / 1000), 3, "V"))
                .unwrap_or_else(|| label("--"))
        };
        set_changed!(get_cc1, set_cc1, line_value(0));
        set_changed!(get_cc2, set_cc2, line_value(1));
        set_changed!(get_dp, set_dp, line_value(2));
        set_changed!(get_dm, set_dm, line_value(3));
        let state = crate::pd_service::snapshot();
        let pd = if state.available {
            label(match state.state {
                km003c_interface::PdState::Starting => "START",
                km003c_interface::PdState::Request => "REQUEST",
                km003c_interface::PdState::Ready => "READY",
                km003c_interface::PdState::Detached => "IDLE",
                km003c_interface::PdState::HardReset => "RESET",
                km003c_interface::PdState::Default => "DEFAULT",
                km003c_interface::PdState::Fallback => "FALLBACK",
                km003c_interface::PdState::Waiting => "WAIT",
                km003c_interface::PdState::Attached => "ATTACHED",
                km003c_interface::PdState::Released => "RELEASED",
                km003c_interface::PdState::Reorienting => "ROTATE",
                km003c_interface::PdState::Failed => "FAILED",
            })
        } else {
            label("OFF")
        };
        set_changed!(get_pd_state, set_pd_state, pd);
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            if force_repaint {
                renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
            }
            renderer.render_by_line(&mut lines);
            lines.flush();
            if force_repaint {
                renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
                force_repaint = false;
            }
        });
        if !lit {
            aw.backlight(30).await.unwrap();
            lit = true;
        }
        if stable & 1 != 0 {
            held_exit += 1;
        } else {
            held_exit = 0;
        }
        if held_exit >= 100
            && let Err(e) = crate::enter_loader(0x600d0002).await
        {
            log::error!("Loader entry failed: {e}");
            held_exit = 0;
        }
        embassy_time::Timer::after_millis(20).await;
    }
}
