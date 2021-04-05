#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_kernel_lib::console::{init_global_console, Console};
use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_kernel_lib::printk;
use rumikan_shared::graphics::FrameBufferInfo;

#[no_mangle]
pub extern "C" fn _start(frame_buffer_info: FrameBufferInfo) -> ! {
    let mut frame_buffer = FrameBuffer::new(frame_buffer_info);
    let console = Console::new(
        frame_buffer,
        PixelColor::new(0, 0, 0),
        PixelColor::new(0xff, 0xff, 0xff),
    );
    init_global_console(console);

    printk!("Hello, world!\n");
    frame_buffer.write_mouse_cursor(
        50,
        50,
        PixelColor::new(0xff, 0xff, 0xff),
        PixelColor::new(0xff, 0, 0),
    );

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
