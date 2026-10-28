//! ST7789-compatible 240x240 panel on the board's shared QSPI bus.
//!
//! The transaction sequence follows native boot routines 0xed4/0xc44. Commands
//! go to the panel only while flash CS (PB1) is a pulled-up GPIO and panel CS
//! (PB15) is low. PB10 is QSPI WP; WPOL supplies the panel's D/C level.
//! No native firmware functions are called: the transport is implemented here.
use core::{
    convert::Infallible,
    ptr::{read_volatile as read, write_volatile as write},
};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
const CR: *mut u32 = 0x9c000000 as *mut u32;
const FCR: *mut u32 = 0x9c000008 as *mut u32;
const SR: *const u32 = 0x9c00000c as *const u32;
const DATA: *mut u32 = 0x9c000010 as *mut u32;
pub struct Display;
impl Display {
    /// Configure the shared controller without issuing any external-flash command.
    ///
    /// # Safety
    /// Own QSPI and PB1/2/10/12/13/14/15 exclusively. No task, interrupt or DMA
    /// may access mapped external flash while the controller serves the panel.
    pub unsafe fn new() -> Self {
        unsafe {
            write(0x40048004 as *mut u32, read(0x40048004 as *const u32) & !8);
            write(0x40053bfc as *mut u16, 0xa501);
            write(0x40053c44 as *mut u16, 0x40); // PB1: flash CS pull-up when muxed to GPIO.
            write(0x40053c7c as *mut u16, 3); // PB15 LCD CS, initially high.
            for pin in [1usize, 2, 10, 12, 13, 14] {
                write((0x40053c42 + pin * 4) as *mut u16, 7);
            }
            write(0x40053bfc as *mut u16, 0xa500);
            // Native single-lane serial-flash read configuration. Transactions
            // temporarily enter direct-command mode and restore this value.
            write(CR, 0x00032a8f);
            write(0x9c000004 as *mut u32, 0x2f);
            write(FCR, 0x8572);
            write(0x9c000014 as *mut u32, 0x0b);
        }
        Self
    }
    fn idle() {
        for _ in 0..2_000_000 {
            if unsafe { read(SR) } & 1 == 0 {
                return;
            }
        }
        panic!("QSPI busy timeout");
    }
    // Keep the flash deselected before changing the controller or lowering LCD
    // CS. The PIDRB check deliberately fails closed if pin muxing is wrong.
    fn begin(&mut self) -> u32 {
        Self::idle();
        unsafe {
            let saved = read(CR);
            write(0x40053bfc as *mut u16, 0xa501);
            write(0x40053c46 as *mut u16, 0); // PB1 flash CS: input with pull-up, deselected.
            write(0x40053bfc as *mut u16, 0xa500);
            assert!(read(0x40053810 as *const u16) & 2 != 0, "Flash CS not high");
            write(0x42a702bc as *mut u32, 0); // PB15 LCD CS low.
            write(CR, (saved & 0xffc0fff8) | 0x20020);
            saved
        }
    }
    fn end(&mut self, saved: u32) {
        Self::idle();
        unsafe {
            write(CR, read(CR) & !0x20);
            write(0x42a702bc as *mut u32, 1);
            write(0x40053bfc as *mut u16, 0xa501);
            write(0x40053c46 as *mut u16, 7);
            write(0x40053bfc as *mut u16, 0xa500);
            write(CR, saved);
        }
    }
    // WP is reused as D/C: low for an opcode, high for parameters/pixels.
    // Wait for the previous byte to leave the bus before changing its level.
    fn phase(data: bool) {
        Self::idle();
        unsafe {
            write(FCR, (read(FCR) & !0x40) | if data { 0x40 } else { 0 });
        }
    }
    fn byte(byte: u8) {
        unsafe {
            write(DATA, u32::from(byte));
        }
    }
    pub fn command(&mut self, command: u8, data: &[u8]) {
        let saved = self.begin();
        Self::phase(false);
        Self::byte(command);
        Self::phase(true);
        for &b in data {
            Self::byte(b);
        }
        self.end(saved);
    }
    pub async fn init(&mut self) {
        // Sleep-out needs a real delay. Gamma/power parameters and MADCTL 0x60
        // are copied from the native panel init, rather than a generic module.
        self.command(0x11, &[]);
        embassy_time::Timer::after_millis(120).await;
        for (command, data) in [
            (0x21, &[][..]),
            (0x36, &[0x60][..]),
            (0x3a, &[5][..]),
            (0xb2, &[0x0c, 0x0c, 0, 0x33, 0x33][..]),
            (0xb7, &[0x75][..]),
            (0xbb, &[0x3f][..]),
            (0xc4, &[0x20][..]),
            (0xc6, &[0x0f][..]),
            (0xd0, &[0xa4, 0][..]),
            (
                0xe0,
                &[0xf0, 8, 14, 9, 8, 4, 47, 51, 85, 54, 20, 19, 42, 45][..],
            ),
            (
                0xe1,
                &[0xf0, 14, 18, 12, 10, 21, 55, 50, 83, 25, 23, 24, 43, 47][..],
            ),
            (0x29, &[][..]),
        ] {
            self.command(command, data);
        }
        embassy_time::Timer::after_millis(20).await;
    }
    fn rectangle(&mut self, area: Rectangle, color: Rgb565) {
        // CASET/RASET use inclusive, big-endian endpoints. Clip before casting
        // signed graphics coordinates; a negative point must never wrap to u16.
        let clipped = area.intersection(&Rectangle::new(Point::zero(), self.size()));
        let Some(end) = clipped.bottom_right() else {
            return;
        };
        let x = clipped.top_left.x as u16;
        let y = clipped.top_left.y as u16;
        let saved = self.begin();
        for (cmd, lo, hi) in [(0x2a, x, end.x as u16), (0x2b, y, end.y as u16)] {
            Self::phase(false);
            Self::byte(cmd);
            Self::phase(true);
            for pair in [lo.to_be_bytes(), hi.to_be_bytes()] {
                for b in pair {
                    Self::byte(b);
                }
            }
        }
        Self::phase(false);
        Self::byte(0x2c);
        Self::phase(true);
        // COLMOD=5 selects RGB565; transmit its high byte first.
        let [high, low] = color.into_storage().to_be_bytes();
        for _ in 0..clipped.size.width * clipped.size.height {
            Self::byte(high);
            Self::byte(low);
        }
        self.end(saved);
    }
}
impl OriginDimensions for Display {
    fn size(&self) -> Size {
        Size::new(240, 240)
    }
}
impl DrawTarget for Display {
    type Color = Rgb565;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, color) in pixels {
            self.rectangle(Rectangle::new(point, Size::new(1, 1)), color);
        }
        Ok(())
    }
    // A full rectangle is one bus transaction, avoiding a framebuffer and the
    // command/mux overhead of setting an address window for every pixel.
    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Infallible> {
        self.rectangle(*area, color);
        Ok(())
    }
}
