#![no_std]
#![no_main]

use core::ffi::c_void;
use core::panic::PanicInfo;

#[repr(C)]
pub struct SimpleTextOutputProtocol {
    dummy: *const c_void,
    output_string: unsafe extern "C" fn(this: *const SimpleTextOutputProtocol,
                                        string: *const u16) -> u64,
}

#[repr(C)]
pub struct SystemTable {
    dummy: [u8;52],
    console_out_handle: *const c_void,
    con_out: *const SimpleTextOutputProtocol,
}

#[no_mangle]
pub unsafe extern "C" fn efi_main(_image_handle: *const c_void,
                                  system_table: *const SystemTable) -> u64 {
    ((*(*system_table).con_out).output_string)(
        (*system_table).con_out,
        ['H' as u16,
            'e' as u16,
            'l' as u16,
            'l' as u16,
            'o' as u16,
            ',' as u16,
            'w' as u16,
            'o' as u16,
            'r' as u16,
            'l' as u16,
            'd' as u16,
            '\n' as u16].as_ptr()
    );
    loop {}
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}
