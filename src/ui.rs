//! Small retained-state diagnostic UI: no heap and no charging-control writes.
use core::fmt::Write;
use embassy_hc32::gpio::{InputPin, Port};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
#[embassy_executor::task]
pub async fn task() {
    let mut aw = unsafe { crate::aw9523::Aw9523::new() };
    aw.lcd_reset().await.unwrap();
    aw.backlight(0).await.unwrap();
    let mut display = unsafe { crate::display::Display::new() };
    display.init().await;
    display.clear(Rgb565::BLACK).unwrap();
    aw.backlight(30).await.unwrap();
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
    let mut redraw = true;
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
        // Accept a change only after three equal 20 ms samples. Count rising
        // edges of the active-low keys, not every sample while a key is held.
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
            redraw = true;
            log::info!("Buttons mask={stable:02x}; presses={counts:?}");
            unsafe {
                (0x200f0d20 as *mut u32).write_volatile(u32::from(stable));
                for (i, count) in counts.iter().enumerate() {
                    (0x200f0d24 as *mut u32).add(i).write_volatile(*count);
                }
            }
        }
        // Redraw on input changes only, so SPI traffic does not monopolize the
        // executor and the independent USB task keeps making progress.
        if redraw {
            #[cfg(feature = "measurement")]
            {
                last_measurement = None;
            }
            display.clear(Rgb565::BLACK).unwrap();
            Text::new(
                "KM003C / Rust",
                Point::new(12, 18),
                MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
            )
            .draw(&mut display)
            .unwrap();
            Text::new(
                "embedded-graphics",
                Point::new(12, 32),
                MonoTextStyle::new(&FONT_6X10, Rgb565::CYAN),
            )
            .draw(&mut display)
            .unwrap();
            for (i, label) in ["EXIT", "OK", "LEFT", "RIGHT"].iter().enumerate() {
                let y = 48 + i as i32 * 40;
                let pressed = stable & (1 << i) != 0;
                Rectangle::new(Point::new(12, y), Size::new(216, 34))
                    .into_styled(PrimitiveStyle::with_fill(if pressed {
                        Rgb565::GREEN
                    } else {
                        Rgb565::BLUE
                    }))
                    .draw(&mut display)
                    .unwrap();
                let mut text = heapless::String::<32>::new();
                write!(text, "{label:5} count {}", counts[i]).unwrap();
                Text::new(
                    &text,
                    Point::new(20, y + 22),
                    MonoTextStyle::new(
                        &FONT_6X10,
                        if pressed {
                            Rgb565::BLACK
                        } else {
                            Rgb565::WHITE
                        },
                    ),
                )
                .draw(&mut display)
                .unwrap();
            }
            Text::new(
                "Hold EXIT 2s -> loader",
                Point::new(12, 230),
                MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
            )
            .draw(&mut display)
            .unwrap();
            redraw = false;
        }
        #[cfg(feature = "measurement")]
        {
            let sample = crate::measurement::snapshot();
            if last_measurement != Some(sample) {
                // Update only the subtitle: a new ADC sample must not force a
                // whole-screen fill and delay the independent USB task.
                Rectangle::new(Point::new(12, 22), Size::new(216, 14))
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                    .draw(&mut display)
                    .unwrap();
                let mut text = heapless::String::<40>::new();
                use crate::measurement::State;
                match sample {
                    State::Probing => write!(text, "ADC probing"),
                    State::BusError => write!(text, "ADC bus error"),
                    State::WrongDevice {
                        manufacturer,
                        device,
                    } => write!(text, "ADC IDs {manufacturer:04x}/{device:04x}"),
                    State::Sample(value) => write!(
                        text,
                        "{}mV Iraw {}{}",
                        value.bus_nanovolts() / 1_000_000,
                        value.current_counts,
                        if value.math_overflow() { " !" } else { "" }
                    ),
                }
                .unwrap();
                Text::new(
                    &text,
                    Point::new(12, 32),
                    MonoTextStyle::new(&FONT_6X10, Rgb565::CYAN),
                )
                .draw(&mut display)
                .unwrap();
                last_measurement = Some(sample);
            }
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
