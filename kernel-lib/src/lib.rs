#![feature(asm)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
#![cfg_attr(not(test), no_std)]

#[macro_use]
pub mod console;
pub mod graphics;
pub mod logger;
pub mod pci;
pub mod usb;
pub mod util;

#[macro_use]
extern crate log;

// re-export macros
pub use log::{debug, error, info, trace, warn};
