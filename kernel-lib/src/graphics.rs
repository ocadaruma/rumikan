use core::fmt;
use core::fmt::{Arguments, Write};

use crate::util::ArrayVec;
use rumikan_shared::graphics::{FrameBufferInfo, PixelFormat};

pub mod fonts {
    use core::slice::from_raw_parts;

    // Font binary should be embedded in kernel ELF
    extern "C" {
        static _binary_shinonome_halfwidth_bin_start: u8;
        static _binary_shinonome_halfwidth_bin_size: u8;
    }

    pub struct Font(*const u8);

    impl Font {
        pub const WIDTH: usize = 8;
        pub const HEIGHT: usize = 16;

        pub fn bytes(&self) -> &[u8] {
            unsafe { from_raw_parts(self.0, 16) }
        }
    }

    pub fn get_font(c: char) -> Option<Font> {
        let size = (unsafe { &_binary_shinonome_halfwidth_bin_size } as *const u8) as u32;
        if let Some(index) = (c as u32).checked_mul(16) {
            if index < size {
                let start_ptr = unsafe { &_binary_shinonome_halfwidth_bin_start } as *const u8;
                return Some(Font(unsafe { start_ptr.offset(index as isize) }));
            }
        }
        None
    }
}

pub mod mouse {
    pub const CURSOR_GLYPH: [u64; 24] = [
        0x1000000000000000,
        0x1100000000000000,
        0x1210000000000000,
        0x1221000000000000,
        0x1222100000000000,
        0x1222210000000000,
        0x1222221000000000,
        0x1222222100000000,
        0x1222222210000000,
        0x1222222221000000,
        0x1222222222100000,
        0x1222222222210000,
        0x1222222222221000,
        0x1222222222222100,
        0x1222222111111110,
        0x1222222100000000,
        0x1222211210000000,
        0x1222101210000000,
        0x1221000121000000,
        0x1210000121000000,
        0x1100000012100000,
        0x1000000012100000,
        0x0000000001210000,
        0x0000000001110000,
    ];
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PixelColor {
    r: u8,
    g: u8,
    b: u8,
}

impl PixelColor {
    pub fn new(r: u8, g: u8, b: u8) -> PixelColor {
        PixelColor { r, g, b }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FrameBuffer(FrameBufferInfo);

impl FrameBuffer {
    pub fn new(info: FrameBufferInfo) -> FrameBuffer {
        FrameBuffer(info)
    }

    pub fn resolution(&self) -> (usize, usize) {
        self.0.resolution()
    }

    pub fn stride(&self) -> usize {
        self.0.stride()
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.0.pixel_format()
    }

    pub fn write_pixel(&mut self, x: usize, y: usize, color: PixelColor) {
        if !(x < self.resolution().0 && y < self.resolution().1) {
            return;
        }

        let pos = (self.stride() * y + x) as isize;
        unsafe {
            let pixel_ptr = self.0.mut_ptr().offset(pos * 4);
            match self.pixel_format() {
                PixelFormat::Rgb => {
                    pixel_ptr.offset(0).write(color.r);
                    pixel_ptr.offset(1).write(color.g);
                    pixel_ptr.offset(2).write(color.b);
                }
                PixelFormat::Bgr => {
                    pixel_ptr.offset(0).write(color.b);
                    pixel_ptr.offset(1).write(color.g);
                    pixel_ptr.offset(2).write(color.r);
                }
            };
        }
    }

    pub fn write_char(&mut self, x: usize, y: usize, c: char, color: PixelColor) {
        if let Some(font) = fonts::get_font(c) {
            for (dy, row) in font.bytes().iter().enumerate() {
                for dx in 0..8 {
                    if (row << dx) & 0x80 != 0 {
                        self.write_pixel(x + dx, y + dy, color);
                    }
                }
            }
        }
    }

    pub fn write_str(&mut self, x: usize, y: usize, s: &str, color: PixelColor) {
        for (i, c) in s.chars().enumerate() {
            self.write_char(x + fonts::Font::WIDTH * i, y, c, color);
        }
    }

    pub fn write_mouse_cursor(
        &mut self,
        x: usize,
        y: usize,
        edge_color: PixelColor,
        fill_color: PixelColor,
    ) {
        for (dy, &row) in mouse::CURSOR_GLYPH.iter().enumerate() {
            for dx in 0..16 {
                match (row >> (4 * (0xf - dx))) & 0xf {
                    // edge
                    1 => self.write_pixel(x + dx, y + dy, edge_color),
                    2 => self.write_pixel(x + dx, y + dy, fill_color),
                    _ => {}
                }
            }
        }
    }

    pub fn write_fmt(
        &mut self,
        x: usize,
        y: usize,
        args: Arguments,
        color: PixelColor,
    ) -> fmt::Result {
        let mut v = CharVec::new();
        v.write_fmt(args)?;

        for (i, &c) in v.as_slice().iter().enumerate() {
            self.write_char(x + fonts::Font::WIDTH * i, y, c, color);
        }
        Ok(())
    }
}

pub type CharVec = ArrayVec<char, 256>;

impl fmt::Write for CharVec {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if self.push(c).is_err() {
                return fmt::Result::Err(fmt::Error);
            }
        }
        fmt::Result::Ok(())
    }
}

impl Default for CharVec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use crate::graphics::CharVec;

    #[test]
    fn char_vec_write_partial() {
        let mut v = CharVec::new();
        for _ in 0..255 {
            v.push('A').unwrap();
        }
        assert!(v.write_str("BCCCCCCC").is_err());
        // must be written partially even if failed to write entire string
        assert_eq!(v.as_slice()[255], 'B');
    }
}
