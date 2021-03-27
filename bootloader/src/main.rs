#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate uefi;
extern crate uefi_services;

use uefi::prelude::*;
use core::fmt::Write;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileMode, FileType, FileAttribute, RegularFile};
use uefi::table::boot::MemoryType;
use core::cell::UnsafeCell;
use uefi::proto::loaded_image::{LoadedImage, DevicePath};

#[entry]
fn efi_main(image_handle: uefi::Handle,
            system_table: SystemTable<Boot>) -> Status {
    let mut buf = [0u8;4096 * 4];
    let bt = system_table.boot_services();
    let fs = get_image_fs(bt, image_handle)?
        .expect("Failed to retrieve `SimpleFileSystem` on device");
    let fs = unsafe { &mut *fs.get() };
    match fs
        .open_volume()
        .expect_success("Failed to open volume")
        .open("\\memmap", FileMode::CreateReadWrite, FileAttribute::empty())
        .expect_success("Failed to open file")
        .into_type()
        .expect_success("Failed to get regular file") {
        FileType::Regular(file) => {
            let (_k, desc_iter) = bt.memory_map(&mut buf)
                .expect_success("Failed to get memory map");
            let mut writer = FileWriter(file);
            writer.write_str("index,type,type(name),physical_start,num_pages,attribute\n").unwrap();
            for (i, desc) in desc_iter.enumerate() {
                writer.write_fmt(
                    format_args!("{},{},{},{},{}\n",
                                 i, desc.ty.name(), desc.phys_start, desc.page_count, desc.att.bits())).unwrap();
            }
        }
        _ => {}
    }
    loop {}
}

trait Display {
    fn name(&self) -> &str;
}

impl Display for MemoryType {
    fn name(&self) -> &str {
        match *self {
            MemoryType::RESERVED => "reserved",
            MemoryType::LOADER_CODE => "loader_code",
            MemoryType::LOADER_DATA => "loader_data",
            MemoryType::BOOT_SERVICES_CODE => "boot_services_code",
            MemoryType::BOOT_SERVICES_DATA => "boot_services_data",
            MemoryType::RUNTIME_SERVICES_CODE => "runtime_services_code",
            MemoryType::RUNTIME_SERVICES_DATA => "runtime_services_data",
            MemoryType::CONVENTIONAL => "conventional",
            MemoryType::UNUSABLE => "unusable",
            MemoryType::ACPI_RECLAIM => "acpi_reclaim",
            MemoryType::ACPI_NON_VOLATILE => "acpi_non_volatile",
            MemoryType::MMIO => "mmio",
            MemoryType::MMIO_PORT_SPACE => "mmio_port_space",
            MemoryType::PAL_CODE => "pal_code",
            MemoryType::PERSISTENT_MEMORY => "persistent_memory",
            _ => "unknown",
        }
    }
}

fn get_image_fs(bt: &BootServices, image_handle: Handle) -> uefi::Result<&UnsafeCell<SimpleFileSystem>> {
    let loaded_image = bt.handle_protocol::<LoadedImage>(image_handle)?
        .expect("Failed to retrieve `LoadedImage` protocol from handle");
    let loaded_image = unsafe { &*loaded_image.get() };

    let device_handle = loaded_image.device();

    let device_path = bt.handle_protocol::<DevicePath>(device_handle)?
        .expect("Failed to retrieve `DevicePath` protocol from image's device handle");

    let device_path = unsafe { &mut *device_path.get() };

    let device_handle = bt.locate_device_path::<SimpleFileSystem>(device_path)?
        .expect("Failed to locate `SimpleFileSystem` protocol on device");

    bt.handle_protocol::<SimpleFileSystem>(device_handle)
}

struct FileWriter(RegularFile);

impl Write for FileWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write(s.as_bytes()).expect_success("Failed to write to regular file");
        core::fmt::Result::Ok(())
    }
}
