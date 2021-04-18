#![feature(asm)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
#![cfg_attr(not(test), no_std)]

#[macro_use]
pub mod console;
pub mod graphics;
pub mod pci;
pub mod usb;
pub mod util;
