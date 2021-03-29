#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate uefi;
extern crate uefi_services;

use uefi::prelude::*;
use core::fmt::Write;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileMode, FileType, FileAttribute, RegularFile, FileInfo};
use uefi::table::boot::{MemoryType, AllocateType};
use core::cell::UnsafeCell;
use uefi::proto::loaded_image::{LoadedImage, DevicePath};

const MEMORY_MAP_FILE: &str = "\\memmap.csv";
const PAGE_SIZE: usize = 0x1000;
const KERNEL_BASE_ADDRESS: usize = 0x100000;
const KERNEL_FILE: &str = "\\rumikan-kernel";

#[entry]
fn efi_main(image_handle: uefi::Handle,
            system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table)
        .expect_success("Failed to initialize");

    system_table.stdout().reset(false)
        .expect_success("Failed to reset text output");

    system_table.stdout().write_str("RuMikan bootloader started.\n")
        .unwrap();

    let bt = system_table.boot_services();
    let fs = get_image_fs(bt, image_handle)
        .expect_success("Failed to retrieve `SimpleFileSystem` on device");
    let fs = unsafe { &mut *fs.get() };

    dump_memory_map(bt, fs);

    let entry_addr = load_kernel_file(bt, fs);

    system_table.stdout().write_fmt(
        format_args!("kernel entry_addr: 0x{:x}\n", entry_addr))
        .unwrap();

    let entry_point: extern "C" fn() = unsafe { core::mem::transmute(entry_addr) };

    let mut mmap_buf = [0u8;4096 * 4];
    system_table.exit_boot_services(image_handle, &mut mmap_buf)
        .expect_success("Failed to exit boot services");

    entry_point();

    loop {}
}

fn format_memory_type(ty: MemoryType) -> &'static str {
    match ty {
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

/// Dump memory map as a file in CSV format.
fn dump_memory_map(bt: &BootServices, fs: &mut SimpleFileSystem) {
    let mut buf = [0u8;4096 * 4];
    let file = open_regular_file(fs, MEMORY_MAP_FILE, FileMode::CreateReadWrite);

    let (_k, desc_iter) = bt.memory_map(&mut buf)
        .expect_success("Failed to get memory map");
    let mut writer = FileWriter(file);

    writer.write_str("index,type,type(name),physical_start,num_pages,attribute\n")
        .unwrap();
    for (i, desc) in desc_iter.enumerate() {
        writer.write_fmt(
            format_args!("{},{},{},{},{}\n",
                         i, format_memory_type(desc.ty), desc.phys_start, desc.page_count, desc.att.bits()))
            .unwrap();
    }
}

/// Load kernel file into memory by allocating pages.
/// Returns the entry-point address of the kernel.
fn load_kernel_file(bt: &BootServices, fs: &mut SimpleFileSystem) -> u64 {
    let mut file = open_regular_file(fs, KERNEL_FILE, FileMode::Read);

    let mut buf = [0u8;1024];
    let info = file.get_info::<FileInfo>(&mut buf)
        .expect_success("Failed to get file info");

    let addr = bt
        .allocate_pages(AllocateType::Address(KERNEL_BASE_ADDRESS),
                        MemoryType::LOADER_DATA,
                        (info.file_size() as usize + 0xfff) / PAGE_SIZE)
        .expect_success("Failed to allocate pages");

    let buf = unsafe {
        core::slice::from_raw_parts_mut(addr as *mut u8, info.file_size() as usize)
    };
    file.read(buf)
        .expect_success("Failed to read kernel file");

    // in ELF, the entry-point address is stored at offset 24
    unsafe { *((addr + 24) as *const u64) }
}

/// Open specified file as RegularFile
fn open_regular_file(fs: &mut SimpleFileSystem, filename: &str, mode: FileMode) -> RegularFile {
    match fs
        .open_volume()
        .expect_success("Failed to open volume")
        .open(filename, mode, FileAttribute::empty())
        .expect_success("Failed to open file")
        .into_type()
        .expect_success("Failed to convert file") {
        FileType::Regular(file) => Some(file),
        _ => None,
    }.expect("Unexpected file type")
}

/// Retrieves the `SimpleFileSystem` protocol associated with
/// the device the given image was loaded from.
///
/// The code is taken from https://github.com/rust-osdev/uefi-rs/blob/724d1d64c6641f1af0735f049d967310665cf0b8/src/table/boot.rs#L571
fn get_image_fs(bt: &BootServices, image_handle: Handle) -> uefi::Result<&UnsafeCell<SimpleFileSystem>> {
    let loaded_image = bt.handle_protocol::<LoadedImage>(image_handle)
        .expect_success("Failed to retrieve `LoadedImage` protocol from handle");
    let loaded_image = unsafe { &*loaded_image.get() };

    let device_handle = loaded_image.device();

    let device_path = bt.handle_protocol::<DevicePath>(device_handle)
        .expect_success("Failed to retrieve `DevicePath` protocol from image's device handle");

    let device_path = unsafe { &mut *device_path.get() };

    let device_handle = bt.locate_device_path::<SimpleFileSystem>(device_path)
        .expect_success("Failed to locate `SimpleFileSystem` protocol on device");

    bt.handle_protocol::<SimpleFileSystem>(device_handle)
}

/// An wrapper for RegularFile to enable writing formatted string
struct FileWriter(RegularFile);

impl Write for FileWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write(s.as_bytes()).expect_success("Failed to write to regular file");
        core::fmt::Result::Ok(())
    }
}
