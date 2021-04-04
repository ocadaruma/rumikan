#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_shared::graphics::FrameBufferInfo;

#[no_mangle]
pub extern "C" fn _start(frame_buffer_info: FrameBufferInfo) -> ! {
    let mut frame_buffer = FrameBuffer::new(frame_buffer_info);

    let bg_color = PixelColor::new(0xbb, 0xbb, 0xbb);
    let fg_color = PixelColor::new(0, 0, 0);

    let (w, h) = frame_buffer.resolution();
    for x in 0..w {
        for y in 0..h {
            frame_buffer.write_pixel(x, y, bg_color);
        }
    }
    for (i, c) in ('!'..='~').enumerate() {
        frame_buffer.write_ascii(8 * i, 50, c, fg_color);
    }
    frame_buffer.write_str(50, 66, "Hello, world!", fg_color);
    frame_buffer.write_fmt(50, 82, format_args!("Background color: {:?}", bg_color), fg_color)
        .unwrap();

    loop {
        unsafe { asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
