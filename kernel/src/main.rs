#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_shared::graphics::FrameBufferInfo;

use crate::graphics::{FrameBuffer, PixelColor};

mod graphics;

#[no_mangle]
pub extern "C" fn _start(frame_buffer_info: FrameBufferInfo) -> ! {
    let mut frame_buffer = FrameBuffer::new(frame_buffer_info);

    let (w, h) = frame_buffer.resolution();
    for x in 0..w {
        for y in 0..h {
            frame_buffer.write_pixel(x, y, PixelColor::new(0xbb, 0xbb, 0xbb));
        }
    }
    frame_buffer.write_ascii(50, 50, 'A', PixelColor::new(0x0, 0x0, 0x0));
    frame_buffer.write_ascii(58, 50, 'A', PixelColor::new(0x0, 0x0, 0x0));

    loop {
        unsafe { asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
