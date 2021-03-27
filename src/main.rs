#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate rlibc;
extern crate uefi;
extern crate uefi_services;

use uefi::prelude::*;
use core::fmt::Write;

#[entry]
fn efi_main(_image_handler: uefi::Handle,
            system_table: SystemTable<Boot>) -> Status {
    system_table.stdout().write_str("Hello, World").unwrap();
    loop {}
}
