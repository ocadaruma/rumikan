#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use rumikan_shared::graphics::{FrameBuffer, PixelColor};

#[no_mangle]
pub extern "C" fn _start(mut frame_buffer: FrameBuffer) -> ! {
    let (w, h) = frame_buffer.resolution();
    for x in 0..w {
        for y in 0..h {
            frame_buffer.write_pixel(x, y, PixelColor::new(0xff, 0xff, 0xff));
        }
    }
    loop {
        unsafe { asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
