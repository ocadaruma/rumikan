#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}

#[repr(C)]
#[derive(Debug)]
pub struct FrameBuffer {
    ptr: *mut u8,
    res_h: usize,
    res_v: usize,
    stride: usize,
    pixel_format: PixelFormat,
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

impl FrameBuffer {
    pub fn new(ptr: *mut u8,
               res_h: usize,
               res_v: usize,
               stride: usize,
               pixel_format: PixelFormat) -> FrameBuffer {
        FrameBuffer { ptr, res_h, res_v, stride, pixel_format, }
    }

    pub fn resolution(&self) -> (usize, usize) {
        (self.res_h, self.res_v)
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn write_pixel(&mut self, x: usize, y: usize, color: PixelColor) {
        let pos = (self.stride * y + x) as isize;
        unsafe {
            let pixel_ptr = self.ptr.offset(pos * 4);
            match self.pixel_format {
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
}
