#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use rumikan_kernel_lib::console::{init_global_console, Console};
use rumikan_kernel_lib::graphics::{FrameBuffer, PixelColor};
use rumikan_kernel_lib::logger::{init_logger, LogLevel};
use rumikan_kernel_lib::pci::{ClassCode, Pci};
use rumikan_kernel_lib::usb::Xhc;
use rumikan_shared::graphics::FrameBufferInfo;

#[macro_use]
extern crate rumikan_kernel_lib;

#[no_mangle]
pub extern "C" fn _start(frame_buffer_info: FrameBufferInfo) -> ! {
    let mut frame_buffer = FrameBuffer::new(frame_buffer_info);
    let console = Console::new(
        frame_buffer,
        PixelColor::new(0, 0, 0),
        PixelColor::new(0xff, 0xff, 0xff),
    );
    init_global_console(console);
    init_logger(LogLevel::Info);

    let mouse_cursor_info = MouseCursorInfo {
        frame_buffer,
        current_pos: (50, 50),
        fill_color: PixelColor::new(0xff, 0, 0),
        edge_color: PixelColor::new(0xff, 0xff, 0xff),
        bgcolor: PixelColor::new(0, 0, 0),
    };
    unsafe {
        MOUSE_CURSOR_INFO = Some(mouse_cursor_info);
    }
    info!("Hello, world!");
    frame_buffer.write_mouse_cursor(
        mouse_cursor_info.current_pos.0,
        mouse_cursor_info.current_pos.1,
        mouse_cursor_info.edge_color,
        mouse_cursor_info.fill_color,
    );
    rumikan_kernel_lib::usb::classdriver::set_default_mouse_observer(on_mouse_event);

    let mut pci = Pci::new();
    if pci.scan_all_bus().is_err() {
        error!("Failed to scan PCI bus");
    }

    for &dev in pci.devices() {
        let vendor_id = dev.read_vendor_id();
        trace!(
            "{}.{}.{}: vend 0x{:04x}, head 0x{:-2x}",
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
        trace!(
            "xHC has been found: {}.{}.{}",
            dev.bus,
            dev.device,
            dev.function
        );
        let xhc_mmio_base = dev.read_bar(0).unwrap() & !0xfusize;
        trace!("xHC mmio_base = 0x{:08x}", xhc_mmio_base);
        dev.switch_ehci2xhci_if_necessary(&pci);

        let mut xhc = Xhc::new(xhc_mmio_base);
        xhc.initialize();
        xhc.run();

        for i in 1..=xhc.max_ports() {
            let mut port = xhc.port_at(i);
            trace!(
                "Port {} is_connected={}",
                port.port_num(),
                port.is_connected()
            );

            if port.is_connected() {
                if let Err(err) = xhc.configure_port(&mut port) {
                    error!("Failed to configure {} due to {:?}", port.port_num(), err);
                }
            }
        }

        loop {
            if let Err(err) = xhc.process_event() {
                error!("Error while process event: {:?}", err);
            }
        }
    }

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[derive(Copy, Clone)]
struct MouseCursorInfo {
    frame_buffer: FrameBuffer,
    current_pos: (usize, usize),
    fill_color: PixelColor,
    edge_color: PixelColor,
    bgcolor: PixelColor,
}

static mut MOUSE_CURSOR_INFO: Option<MouseCursorInfo> = None;

fn on_mouse_event(delta: (i8, i8)) {
    let mut info = unsafe { MOUSE_CURSOR_INFO }.unwrap();
    info.frame_buffer
        .erase_mouse_cursor(info.current_pos.0, info.current_pos.1, info.bgcolor);
    let (x, y) = info.current_pos;
    let (mut x, mut y) = (x as isize, y as isize);
    x += delta.0 as isize;
    y += delta.1 as isize;

    let (x, y) = (x as usize, y as usize);
    info.current_pos = (x, y);
    info.frame_buffer
        .write_mouse_cursor(x, y, info.edge_color, info.fill_color);
    unsafe { MOUSE_CURSOR_INFO = Some(info) };
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
