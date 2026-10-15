use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::BinaryColor,
    Pixel,
};

pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 43;
pub const PACKET_LEN: usize = 992;
pub const DATA_LEN: usize = 960; // 6 blocks * 160 columns

#[derive(Clone, Debug)]
pub struct G510Framebuffer {
    pixels: [bool; WIDTH * HEIGHT],
}

impl Default for G510Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl G510Framebuffer {
    pub const fn new() -> Self {
        Self {
            pixels: [false; WIDTH * HEIGHT],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, on: bool) {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x] = on;
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x]
        } else {
            false
        }
    }

    /// Packs the 160x43 framebuffer into the 992-byte G510 HID Output Report 3.
    ///
    /// Layout:
    /// - Byte 0: 0x03 (Report ID 3)
    /// - Bytes 1..31: 0x00 (padding)
    /// - Bytes 32..991: 960 bytes representing 6 vertical blocks of 8 pixels across 160 columns.
    ///   Within each byte, bit 0 is top pixel (y = block*8), bit 7 is bottom pixel.
    pub fn pack_report(&self, invert: bool) -> [u8; PACKET_LEN] {
        let mut report = [0u8; PACKET_LEN];
        report[0] = 0x03;

        let mut offset = 32;
        for block in 0..6 {
            for x in 0..WIDTH {
                let mut byte_val = 0u8;
                for bit in 0..8 {
                    let y = block * 8 + bit;
                    if y < HEIGHT {
                        let is_on = self.get_pixel(x, y);
                        let active = if invert { !is_on } else { is_on };
                        if active {
                            byte_val |= 1 << bit;
                        }
                    }
                }
                report[offset] = byte_val;
                offset += 1;
            }
        }
        report
    }
}

impl OriginDimensions for G510Framebuffer {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for G510Framebuffer {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), core::convert::Infallible>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(Point { x, y }, color) in pixels.into_iter() {
            if x >= 0 && (x as usize) < WIDTH && y >= 0 && (y as usize) < HEIGHT {
                self.set_pixel(x as usize, y as usize, color.is_on());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_and_defaults() {
        let fb = G510Framebuffer::new();
        assert_eq!(fb.size(), Size::new(160, 43));
        assert!(!fb.get_pixel(0, 0));
        assert!(!fb.get_pixel(159, 42));
    }

    #[test]
    fn test_pixel_set_get() {
        let mut fb = G510Framebuffer::new();
        fb.set_pixel(10, 20, true);
        assert!(fb.get_pixel(10, 20));
        assert!(!fb.get_pixel(10, 21));
        fb.clear();
        assert!(!fb.get_pixel(10, 20));
    }

    #[test]
    fn test_pack_report_header_and_length() {
        let fb = G510Framebuffer::new();
        let report = fb.pack_report(false);
        assert_eq!(report.len(), PACKET_LEN);
        assert_eq!(report[0], 0x03);
        for b in &report[1..32] {
            assert_eq!(*b, 0x00);
        }
        for b in &report[32..992] {
            assert_eq!(*b, 0x00);
        }
    }

    #[test]
    fn test_pack_report_pixel_mapping() {
        let mut fb = G510Framebuffer::new();
        // Set pixel at (0, 0) -> Block 0, bit 0, byte offset 32
        fb.set_pixel(0, 0, true);
        // Set pixel at (0, 7) -> Block 0, bit 7, byte offset 32
        fb.set_pixel(0, 7, true);
        // Set pixel at (1, 8) -> Block 1, bit 0, byte offset 32 + 160 + 1 = 193
        fb.set_pixel(1, 8, true);

        let report = fb.pack_report(false);
        assert_eq!(report[32], 0b10000001);
        assert_eq!(report[32 + 160 + 1], 0b00000001);
    }
}
