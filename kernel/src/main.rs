#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;

#[no_mangle]
// need "efiapi" to adjust calling convetion so that callable from UEFI bootloader
pub extern "efiapi" fn _start(frame_buffer_addr: u64, frame_buffer_size: usize) -> ! {
    for i in 0..frame_buffer_size {
        let ptr: *mut u8 = unsafe { core::mem::transmute(frame_buffer_addr) };
        unsafe { ptr.offset(i as isize).write(255) };
    }
    loop {
        unsafe { asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
