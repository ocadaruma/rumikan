#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_kernel_lib::console::{init_global_console, Console};
use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_kernel_lib::pci::{ClassCode, Pci};
use rumikan_kernel_lib::printk;
use rumikan_kernel_lib::usb::Xhc;
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

    for &dev in pci.devices() {
        let vendor_id = dev.read_vendor_id();
        printk!(
            "{}.{}.{}: vend 0x{:04x}, head 0x{:-2x}\n",
            dev.bus,
            dev.device,
            dev.function,
            vendor_id,
            dev.header_type
        );
    }

    let xhc_class_code = ClassCode {
        base: 0x0c,
        sub: 0x03,
        interface: 0x30,
    };
    let mut xhc_dev = None;
    for &dev in pci.devices() {
        if dev.class_code == xhc_class_code {
            xhc_dev = Some(dev);

            if dev.read_vendor_id() == 0x8086 {
                break;
            }
        }
    }
    if let Some(dev) = xhc_dev {
        printk!(
            "xHC has been found: {}.{}.{}\n",
            dev.bus,
            dev.device,
            dev.function
        );
        let xhc_mmio_base = dev.read_bar(0).unwrap() & !0xfusize;
        printk!("xHC mmio_base = 0x{:08x}\n", xhc_mmio_base);
        dev.switch_ehci2xhci_if_necessary(&pci);

        let mut xhc = Xhc::new(xhc_mmio_base);
        xhc.initialize();
        xhc.run();

        for i in 1..=xhc.max_ports() {
            let mut port = xhc.port_at(i);
            printk!(
                "Port {} is_connected={}\n",
                port.port_num(),
                port.is_connected()
            );

            if port.is_connected() {
                if let Err(err) = xhc.configure_port(&mut port) {
                    printk!("Failed to configure {} due to {:?}\n", port.port_num(), err);
                }
            }
        }
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
