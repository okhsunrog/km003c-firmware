//! Small off-screen dirty-region buffer. Compose text/background before sending
//! any pixels to the panel; a full RGB565 frame would cost 115200 bytes.
use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};

pub struct Tile<const N: usize> {
    pixels: [Rgb565; N],
    size: Size,
}
impl<const N: usize> Default for Tile<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> Tile<N> {
    pub const fn new() -> Self {
        Self {
            pixels: [Rgb565::BLACK; N],
            size: Size::new(0, 0),
        }
    }
    pub fn reset(&mut self, size: Size, background: Rgb565) {
        let count = size
            .width
            .checked_mul(size.height)
            .expect("tile dimensions overflow") as usize;
        assert!(count <= N, "region exceeds tile buffer");
        self.size = size;
        self.pixels[..count].fill(background);
    }
    pub fn pixels(&self) -> &[Rgb565] {
        &self.pixels[..(self.size.width * self.size.height) as usize]
    }
}
impl<const N: usize> OriginDimensions for Tile<N> {
    fn size(&self) -> Size {
        self.size
    }
}
impl<const N: usize> DrawTarget for Tile<N> {
    type Color = Rgb565;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, color) in pixels {
            if self.bounding_box().contains(point) {
                let index = point.y as usize * self.size.width as usize + point.x as usize;
                self.pixels[index] = color;
            }
        }
        Ok(())
    }
    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Infallible> {
        let area = area.intersection(&self.bounding_box());
        if let Some(end) = area.bottom_right() {
            for y in area.top_left.y..=end.y {
                let first = y as usize * self.size.width as usize + area.top_left.x as usize;
                self.pixels[first..first + area.size.width as usize].fill(color);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clip_and_reset_without_transposing_rows() {
        let mut tile = Tile::<12>::new();
        tile.reset(Size::new(4, 3), Rgb565::BLACK);
        tile.fill_solid(
            &Rectangle::new(Point::new(-1, 1), Size::new(3, 3)),
            Rgb565::RED,
        )
        .unwrap();
        tile.draw_iter([
            Pixel(Point::new(3, 0), Rgb565::GREEN),
            Pixel(Point::new(-1, 0), Rgb565::BLUE),
            Pixel(Point::new(4, 0), Rgb565::BLUE),
        ])
        .unwrap();
        assert_eq!(
            tile.pixels(),
            &[
                Rgb565::BLACK,
                Rgb565::BLACK,
                Rgb565::BLACK,
                Rgb565::GREEN,
                Rgb565::RED,
                Rgb565::RED,
                Rgb565::BLACK,
                Rgb565::BLACK,
                Rgb565::RED,
                Rgb565::RED,
                Rgb565::BLACK,
                Rgb565::BLACK,
            ]
        );
        tile.reset(Size::new(2, 1), Rgb565::BLUE);
        assert_eq!(tile.pixels(), &[Rgb565::BLUE; 2]);
    }
}
