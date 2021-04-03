use rumikan_shared::graphics::{FrameBufferInfo, PixelFormat};

pub mod fonts {
    pub const A: [u8;16] = [
        0b00000000,
        0b00011000,
        0b00011000,
        0b00011000,
        0b00011000,
        0b00100100,
        0b00100100,
        0b00100100,
        0b00100100,
        0b01111110,
        0b01000010,
        0b01000010,
        0b01000010,
        0b11100111,
        0b00000000,
        0b00000000,
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
        PixelColor { r, g, b, }
    }
}

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
                },
                PixelFormat::Bgr => {
                    pixel_ptr.offset(0).write(color.b);
                    pixel_ptr.offset(1).write(color.g);
                    pixel_ptr.offset(2).write(color.r);
                },
            };
        }
    }

    pub fn write_ascii(&mut self, x: usize, y: usize, c: char, color: PixelColor) {
        if let Some(font) = match c {
            'A' => Some(fonts::A),
            _ => None,
        } {
            for (dy, row) in font.iter().enumerate() {
                for dx in 0..8 {
                    if (row << dx) & 0x80 != 0 {
                        self.write_pixel(x + dx, y + dy, color);
                    }
                }
            }
        }
    }
}
