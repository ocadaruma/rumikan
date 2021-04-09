#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_kernel_lib::console::{init_global_console, Console};
use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_kernel_lib::pci::Pci;
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

    let mut pci = Pci::new();
    if pci.scan_all_bus().is_err() {
        printk!("Failed to scan PCI bus\n");
    }
    for &device in pci.devices() {
        let vendor_id = device.read_vendor_id();
        printk!(
            "{}.{}.{}: vend 0x{:04x}, head 0x{:-2x}\n",
            device.bus,
            device.device,
            device.function,
            vendor_id,
            device.header_type
        );
    }

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
