#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameBufferInfo {
    ptr: *mut u8,
    res_h: usize,
    res_v: usize,
    stride: usize,
    pixel_format: PixelFormat,
}

impl FrameBufferInfo {
    pub fn new(ptr: *mut u8,
               res_h: usize,
               res_v: usize,
               stride: usize,
               pixel_format: PixelFormat) -> FrameBufferInfo {
        FrameBufferInfo { ptr, res_h, res_v, stride, pixel_format, }
    }

    pub fn mut_ptr(&mut self) -> *mut u8 {
        self.ptr
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
}
