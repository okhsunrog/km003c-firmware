//! Quarter-turn transforms for streamed rectangular tiles, without a framebuffer.
use embedded_graphics::{prelude::*, primitives::Rectangle};
#[derive(Clone, Copy, Default)]
pub struct Rotation(u8);
impl Rotation {
    pub fn new(turns: u8) -> Self {
        Self(turns % 4)
    }
    pub fn turns(self) -> u8 {
        self.0
    }
    pub fn area(self, area: Rectangle, side: i32) -> Rectangle {
        let (x, y) = (area.top_left.x, area.top_left.y);
        let (w, h) = (area.size.width as i32, area.size.height as i32);
        let (x, y, w, h) = match self.0 {
            0 => (x, y, w, h),
            1 => (side - y - h, x, h, w),
            2 => (side - x - w, side - y - h, w, h),
            _ => (y, side - x - w, h, w),
        };
        Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32))
    }
    /// Source index for each pixel in the rotated window's row-major stream.
    pub fn source(self, index: usize, width: usize, height: usize) -> usize {
        match self.0 {
            0 => index,
            1 => (height - 1 - index % height) * width + index / height,
            2 => width * height - 1 - index,
            _ => (index % height) * width + (width - 1 - index / height),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asymmetric_tile_pixels_and_origin() {
        let expected = [
            [0, 1, 2, 3, 4, 5],
            [3, 0, 4, 1, 5, 2],
            [5, 4, 3, 2, 1, 0],
            [2, 5, 1, 4, 0, 3],
        ];
        for (i, indices) in expected.iter().enumerate() {
            let r = Rotation::new(i as u8);
            assert_eq!(
                core::array::from_fn::<_, 6, _>(|n| r.source(n, 3, 2)),
                *indices
            );
        }
        let a = Rectangle::new(Point::new(2, 4), Size::new(3, 2));
        assert_eq!(
            Rotation::new(1).area(a, 10),
            Rectangle::new(Point::new(4, 2), Size::new(2, 3))
        );
    }
}
