//! Meter screen: large V/A/W values, temperature and calibration state.
//! Compose changed regions in RAM; input diagnostics stay in logs/retention.
#[cfg(feature = "measurement")]
use core::fmt::Write;
use embassy_hc32::gpio::{InputPin, Port};
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use km003c_drivers::tile::Tile;

type UiTile = Tile<{ 216 * 44 }>;
type Value = heapless::String<24>;

// Scale the built-in font in RAM instead of sending separate LCD transactions
// for glyph pixels. The final 40-pixel-high number is one contiguous upload.
struct Double<'a>(&'a mut UiTile);
impl OriginDimensions for Double<'_> {
    fn size(&self) -> Size {
        Size::new(self.0.size().width / 2, self.0.size().height / 2)
    }
}
impl DrawTarget for Double<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(p, c) in pixels {
            self.0.fill_solid(
                &Rectangle::new(Point::new(p.x * 2, p.y * 2), Size::new(2, 2)),
                c,
            )?;
        }
        Ok(())
    }
}
fn label(text: &str) -> Value {
    Value::try_from(text).unwrap()
}
#[cfg(feature = "measurement")]
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
fn upload(display: &mut crate::display::Display, tile: &UiTile, origin: Point) {
    display.blit(Rectangle::new(origin, tile.size()), tile.pixels());
}
fn row(
    display: &mut crate::display::Display,
    tile: &mut UiTile,
    cache: &mut Option<Value>,
    value: &Value,
    y: i32,
    color: Rgb565,
) {
    if cache.as_ref() == Some(value) {
        return;
    }
    tile.reset(Size::new(216, 44), Rgb565::BLACK);
    let x = (108 - (value.len() as i32 * 10)).max(0) / 2;
    Text::new(
        value,
        Point::new(x, 16),
        MonoTextStyle::new(&FONT_10X20, color),
    )
    .draw(&mut Double(tile))
    .unwrap();
    upload(display, tile, Point::new(12, y));
    *cache = Some(value.clone());
}

fn chrome(display: &mut crate::display::Display, tile: &mut UiTile) {
    tile.reset(Size::new(216, 18), Rgb565::BLACK);
    Text::new(
        "KM003C / Rust",
        Point::new(0, 10),
        MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
    )
    .draw(tile)
    .unwrap();
    upload(display, tile, Point::new(12, 8));
    tile.reset(Size::new(216, 10), Rgb565::BLACK);
    Text::new(
        "EXIT 2s: loader",
        Point::new(0, 8),
        MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
    )
    .draw(tile)
    .unwrap();
    upload(display, tile, Point::new(12, 228));
}

#[embassy_executor::task]
pub async fn task() {
    let mut aw = unsafe { crate::aw9523::Aw9523::new() };
    aw.lcd_reset().await.unwrap();
    aw.backlight(0).await.unwrap();
    let mut display = unsafe { crate::display::Display::new() };
    #[cfg(feature = "measurement")]
    {
        let (id, block) = display.factory_settings();
        crate::measurement::factory_settings(id, &block);
    }
    #[cfg(feature = "orientation")]
    let mut orientation = crate::orientation::probe(aw.bus_mut()).await;
    #[cfg(feature = "orientation")]
    let mut rotation = if orientation.is_some() { 3 } else { 0 };
    #[cfg(feature = "orientation")]
    display.set_rotation(rotation);
    display.init().await;
    display.clear(Rgb565::BLACK).unwrap();
    let mut tile = UiTile::new();
    chrome(&mut display, &mut tile);
    let buttons = unsafe {
        [
            InputPin::new_pull_up(Port::B, 7),
            InputPin::new_pull_up(Port::B, 11),
            InputPin::new_pull_up(Port::B, 8),
            InputPin::new_pull_up(Port::B, 9),
        ]
    };
    let mut counts = [0u32; 4];
    let (mut previous, mut stable, mut repeats) = (0u8, 0u8, 0u8);
    let (mut lit, mut held_exit) = (false, 0);
    let mut caches: [Option<Value>; 5] = core::array::from_fn(|_| None);
    #[cfg(feature = "measurement")]
    let mut line_caches: [Option<Value>; 4] = core::array::from_fn(|_| None);
    #[cfg(feature = "measurement")]
    let mut lines_due = embassy_time::Instant::now();
    unsafe {
        for i in 0..8 {
            (0x200f0d20 as *mut u32).add(i).write_volatile(0);
        }
    }
    log::info!("Meter UI ready; hold EXIT PB7 for loader");
    loop {
        #[cfg(feature = "orientation")]
        if let Some(sensor) = orientation.as_mut() {
            let next = sensor.poll(aw.bus_mut()).await;
            if next != rotation {
                rotation = next;
                display.set_rotation(rotation);
                // Each tile remains contiguous on the bus. Clear/repaint only
                // when a debounced quarter-turn changes the complete layout.
                display.clear(Rgb565::BLACK).unwrap();
                caches.fill(None);
                #[cfg(feature = "measurement")]
                line_caches.fill(None);
                chrome(&mut display, &mut tile);
                log::info!("Orientation quarter-turn={rotation}");
            }
        }
        let mut raw = 0;
        for (i, b) in buttons.iter().enumerate() {
            if b.is_low() {
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
            for (i, count) in counts.iter_mut().enumerate() {
                if raw & (1 << i) != 0 && stable & (1 << i) == 0 {
                    *count += 1;
                }
            }
            stable = raw;
            // Keep the existing diagnostic record for host acceptance, but
            // dedicate all screen space to actual measurements.
            log::info!("Buttons mask={stable:02x}; presses={counts:?}");
            unsafe {
                (0x200f0d20 as *mut u32).write_volatile(u32::from(stable));
                for (i, count) in counts.iter().enumerate() {
                    (0x200f0d24 as *mut u32).add(i).write_volatile(*count);
                }
            }
        }
        #[cfg(feature = "measurement")]
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
        #[cfg(not(feature = "measurement"))]
        let values = [
            label("NO DATA"),
            label("-- A"),
            label("-- W"),
            label("-- C"),
            label("OFF"),
        ];
        row(
            &mut display,
            &mut tile,
            &mut caches[0],
            &values[0],
            32,
            Rgb565::CYAN,
        );
        row(
            &mut display,
            &mut tile,
            &mut caches[1],
            &values[1],
            81,
            Rgb565::WHITE,
        );
        row(
            &mut display,
            &mut tile,
            &mut caches[2],
            &values[2],
            130,
            Rgb565::YELLOW,
        );
        if caches[3].as_ref() != Some(&values[3]) {
            tile.reset(Size::new(144, 18), Rgb565::BLACK);
            let mut header = Value::new();
            #[cfg(feature = "measurement")]
            write!(header, "RUST {}", values[3]).unwrap();
            #[cfg(not(feature = "measurement"))]
            header.push_str("KM003C / Rust").unwrap();
            Text::new(
                &header,
                Point::new(0, 10),
                MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
            )
            .draw(&mut tile)
            .unwrap();
            upload(&mut display, &tile, Point::new(12, 8));
            caches[3] = Some(values[3].clone());
        }
        if caches[4].as_ref() != Some(&values[4]) {
            tile.reset(Size::new(64, 18), Rgb565::BLACK);
            Text::new(
                &values[4],
                Point::new(0, 10),
                MonoTextStyle::new(&FONT_6X10, Rgb565::GREEN),
            )
            .draw(&mut tile)
            .unwrap();
            upload(&mut display, &tile, Point::new(164, 8));
            caches[4] = Some(values[4].clone());
        }
        #[cfg(feature = "measurement")]
        if embassy_time::Instant::now() >= lines_due {
            let sample = crate::lines::snapshot();
            for (i, name) in ["CC1", "CC2", "D+ ", "D- "].into_iter().enumerate() {
                let mut text = Value::new();
                if let Some(s) = sample {
                    let mv = s.uv[i] / 1000;
                    write!(text, "{name} {}.{:03}", mv / 1000, mv % 1000).unwrap();
                } else {
                    write!(text, "{name} --").unwrap();
                }
                if line_caches[i].as_ref() != Some(&text) {
                    // Four useful measurements in a two-column footer. Each
                    // changed value is still one contiguous panel transaction.
                    tile.reset(Size::new(108, 20), Rgb565::BLACK);
                    Text::new(
                        &text,
                        Point::new(0, 16),
                        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
                    )
                    .draw(&mut tile)
                    .unwrap();
                    upload(
                        &mut display,
                        &tile,
                        Point::new(12 + (i as i32 % 2) * 108, 181 + (i as i32 / 2) * 22),
                    );
                    line_caches[i] = Some(text);
                }
            }
            lines_due = embassy_time::Instant::now() + embassy_time::Duration::from_millis(250);
        }
        if !lit {
            aw.backlight(30).await.unwrap();
            lit = true;
        }
        if stable & 1 != 0 {
            held_exit += 1;
        } else {
            held_exit = 0;
        }
        if held_exit >= 100 {
            crate::return_to_loader(0x600d0002);
        }
        embassy_time::Timer::after_millis(20).await;
    }
}
