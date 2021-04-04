#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use core::fmt::Arguments;
use rumikan_kernel_lib::console::Console;
use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_shared::graphics::FrameBufferInfo;

static mut CONSOLE: Option<Console> = None;

#[no_mangle]
pub extern "C" fn _start(frame_buffer_info: FrameBufferInfo) -> ! {
    let console = Console::new(
        FrameBuffer::new(frame_buffer_info),
        PixelColor::new(0, 0, 0),
        PixelColor::new(0xff, 0xff, 0xff),
    );
    unsafe {
        CONSOLE = Some(console);
    }

    printk!("Hello, world!\n");

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

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

fn _print(args: Arguments) {
    unsafe {
        CONSOLE.as_mut().unwrap().print(args);
    }
}
