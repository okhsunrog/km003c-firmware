//! Small diagnostic UI: precompose only changed regions, never erase a live
//! full frame on a key press. No heap and no charging-control writes.
use core::fmt::Write;
use embassy_hc32::gpio::{InputPin, Port};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use km003c_drivers::tile::Tile;

type UiTile = Tile<{ 216 * 34 }>;
fn text(tile: &mut UiTile, value: &str, position: Point, color: Rgb565) {
    Text::new(value, position, MonoTextStyle::new(&FONT_6X10, color))
        .draw(tile)
        .unwrap();
}
fn upload(display: &mut crate::display::Display, tile: &UiTile, origin: Point) {
    display.blit(Rectangle::new(origin, tile.size()), tile.pixels());
}

#[embassy_executor::task]
pub async fn task() {
    let mut aw = unsafe { crate::aw9523::Aw9523::new() };
    aw.lcd_reset().await.unwrap();
    aw.backlight(0).await.unwrap();
    let mut display = unsafe { crate::display::Display::new() };
    display.init().await;
    display.clear(Rgb565::BLACK).unwrap();
    // One 14.7 KB tile fits SRAM; a 115.2 KB full framebuffer would leave too
    // little space for USB, the executor and stack. Glyphs are drawn in RAM so
    // each upload is a continuous RGB565 stream, not a transaction per pixel.
    let mut tile = UiTile::new();
    tile.reset(Size::new(216, 28), Rgb565::BLACK);
    text(&mut tile, "KM003C / Rust", Point::new(0, 10), Rgb565::WHITE);
    text(
        &mut tile,
        "embedded-graphics",
        Point::new(0, 24),
        Rgb565::CYAN,
    );
    upload(&mut display, &tile, Point::new(12, 8));
    tile.reset(Size::new(216, 14), Rgb565::BLACK);
    text(
        &mut tile,
        "Hold EXIT 2s -> loader",
        Point::new(0, 10),
        Rgb565::WHITE,
    );
    upload(&mut display, &tile, Point::new(12, 220));
    let buttons = unsafe {
        [
            InputPin::new_pull_up(Port::B, 7),
            InputPin::new_pull_up(Port::B, 11),
            InputPin::new_pull_up(Port::B, 8),
            InputPin::new_pull_up(Port::B, 9),
        ]
    };
    let mut counts = [0u32; 4];
    let mut previous = 0u8;
    let mut stable = 0u8;
    let mut repeats = 0;
    let mut dirty = 0x0fu8;
    let mut lit = false;
    let mut held_exit = 0;
    #[cfg(feature = "measurement")]
    let mut last_measurement = None;
    unsafe {
        for i in 0..8 {
            (0x200f0d20 as *mut u32).add(i).write_volatile(0);
        }
    }
    log::info!("Display ready; buttons EXIT PB7, OK PB11, LEFT PB8, RIGHT PB9");
    loop {
        let mut raw = 0;
        for (i, b) in buttons.iter().enumerate() {
            if b.is_low() {
                raw |= 1 << i;
            }
        }
        // Three repeated 20 ms samples debounce each transition. Count only
        // press edges; both press and release dirty the corresponding row.
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
            dirty |= stable ^ raw;
            stable = raw;
            log::info!("Buttons mask={stable:02x}; presses={counts:?}");
            unsafe {
                (0x200f0d20 as *mut u32).write_volatile(u32::from(stable));
                for (i, count) in counts.iter().enumerate() {
                    (0x200f0d24 as *mut u32).add(i).write_volatile(*count);
                }
            }
        }
        // A key changes one 216x34 region, never the static header/footer or
        // other rows. Send the complete row with text already composed.
        for (i, label) in ["EXIT", "OK", "LEFT", "RIGHT"].iter().enumerate() {
            if dirty & (1 << i) == 0 {
                continue;
            }
            let pressed = stable & (1 << i) != 0;
            tile.reset(
                Size::new(216, 34),
                if pressed { Rgb565::GREEN } else { Rgb565::BLUE },
            );
            let mut value = heapless::String::<32>::new();
            write!(value, "{label:5} count {}", counts[i]).unwrap();
            text(
                &mut tile,
                &value,
                Point::new(8, 22),
                if pressed {
                    Rgb565::BLACK
                } else {
                    Rgb565::WHITE
                },
            );
            upload(&mut display, &tile, Point::new(12, 48 + i as i32 * 40));
        }
        dirty = 0;
        #[cfg(feature = "measurement")]
        {
            let sample = crate::measurement::snapshot();
            let mut value = heapless::String::<40>::new();
            use crate::measurement::State;
            match sample {
                State::Probing => write!(value, "ADC probing"),
                State::BusError => write!(value, "ADC bus error"),
                State::WrongDevice {
                    manufacturer,
                    device,
                } => write!(value, "ADC IDs {manufacturer:04x}/{device:04x}"),
                State::Sample(sample) => {
                    if let Some(current) = crate::measurement::uncalibrated_microamps(sample) {
                        write!(
                            value,
                            "{}mV I0={}mA",
                            sample.bus_nanovolts() / 1_000_000,
                            current / 1000
                        )
                    } else {
                        write!(
                            value,
                            "{}mV Iraw {}{}",
                            sample.bus_nanovolts() / 1_000_000,
                            sample.current_counts,
                            if sample.math_overflow() { " !" } else { "" }
                        )
                    }
                }
            }
            .unwrap();
            // Compare rendered text, not every underlying ADC bit/temperature
            // or conversion-ready flag. Do not erase and redraw equal strings.
            if last_measurement.as_ref() != Some(&value) {
                tile.reset(Size::new(216, 14), Rgb565::BLACK);
                text(&mut tile, &value, Point::new(0, 10), Rgb565::CYAN);
                upload(&mut display, &tile, Point::new(12, 22));
                last_measurement = Some(value);
            }
        }
        if !lit {
            // Reveal the first completed UI, avoiding visible startup clearing.
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
