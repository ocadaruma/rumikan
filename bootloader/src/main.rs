#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use core::cell::UnsafeCell;
use core::fmt::Write;
use core::mem::{size_of, transmute};
use core::slice::{from_raw_parts_mut, from_raw_parts};

use log::info;
use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::loaded_image::{DevicePath, LoadedImage};
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType, RegularFile};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::{AllocateType, MemoryType};
use uefi::table::runtime::Time;

use crate::elf64::SegmentType;

mod elf64;

const MEMORY_MAP_FILE: &str = "\\memmap.csv";
const KERNEL_FILE: &str = "\\rumikan-kernel";
// 4KB
const PAGE_SIZE: usize = 0x1000;
// Calculate required buffer size which aligned to the struct size
const FILE_INFO_BUFFER_LEN: usize = {
    let mut align = size_of::<FileInfoHeader>();
    // 15 = "rumikan-kernel".len() + 1 (null character)
    let required = align + 15 * size_of::<Char16>();
    while align < required { align *= 2; }
    align
};

#[entry]
fn efi_main(image_handle: uefi::Handle,
            system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table)
        .expect_success("Failed to initialize");

    system_table.stdout().reset(false)
        .expect_success("Failed to reset text output");

    info!("RuMikan bootloader started.");

    let bt = system_table.boot_services();
    let fs = get_image_fs(bt, image_handle)
        .expect_success("Failed to retrieve `SimpleFileSystem` on device");
    let fs = unsafe { &mut *fs.get() };

    dump_memory_map(bt, fs);

    let entry_addr = load_kernel_file(bt, fs);

    info!("kernel entry_addr: 0x{:x}", entry_addr);

    let entry_point: extern "sysv64" fn(rumikan_shared::graphics::FrameBuffer) -> ! = unsafe {
        transmute(entry_addr)
    };

    let frame_buffer = get_frame_buffer(bt);

    let mut mmap_buf = [0u8;4096 * 4];
    system_table.exit_boot_services(image_handle, &mut mmap_buf)
        .expect_success("Failed to exit boot services");

    entry_point(frame_buffer);
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
            format_args!("{},{:?},{},{},{}\n",
                         i, desc.ty, desc.phys_start, desc.page_count, desc.att.bits()))
            .unwrap();
    }
}

/// Load kernel file into memory by allocating pages.
/// Returns the entry-point address of the kernel.
fn load_kernel_file(bt: &BootServices, fs: &mut SimpleFileSystem) -> u64 {
    let mut file = open_regular_file(fs, KERNEL_FILE, FileMode::Read);
    let mut buf = [0u8; FILE_INFO_BUFFER_LEN];
    let info = file.get_info::<FileInfo>(&mut buf)
        .expect_success("Failed to get file info");

    let pool = bt.allocate_pool(MemoryType::LOADER_DATA, info.file_size() as usize)
        .expect_success("Failed to allocate pool for load kernel file temporary");
    unsafe {
        file.read(from_raw_parts_mut(pool, info.file_size() as usize))
            .expect_success("Failed to read kernel file");
    }
    let file_header = pool as *const elf64::FileHeader;
    let file_header = unsafe { &*file_header };

    let program_header = unsafe {
        pool.offset(file_header.e_phoff as isize)
    } as *const elf64::ProgramHeader;
    let program_headers = unsafe {
        from_raw_parts(program_header, file_header.e_phnum as usize)
    };
    let (first_addr, last_addr) = program_headers.iter()
        .filter(|h| h.p_type == SegmentType::Load)
        .fold((u64::MAX, 0), |acc, header| {
            (u64::min(acc.0, header.p_vaddr), (u64::max(acc.1, header.p_vaddr + header.p_memsz)))
        });

    let addr = bt.allocate_pages(
        AllocateType::Address(first_addr as usize),
        MemoryType::LOADER_DATA,
        ((last_addr - first_addr + 0xfff) as usize) / PAGE_SIZE)
        .expect_success("Failed to allocate pages");

    program_headers.iter()
        .filter(|h| h.p_type == SegmentType::Load).for_each(|header| {
        unsafe {
            let dest: *mut u8 = transmute(header.p_vaddr);
            let src = pool.offset(header.p_offset as isize);
            dest.copy_from(src, header.p_filesz as usize);

            for i in 0..(header.p_memsz - header.p_filesz) {
                dest.offset((header.p_filesz + i) as isize).write(0);
            }
        }
    });
    bt.free_pool(pool)
        .expect_success("Failed to free pool");

    // in ELF, the entry-point address is stored at offset 24
    unsafe { *((addr + 24) as *const u64) }
}

/// Get frame buffer struct which will be passed to kernel entry point
fn get_frame_buffer(bt: &BootServices) -> rumikan_shared::graphics::FrameBuffer {
    let gop = unsafe {
        &mut *(bt.locate_protocol::<GraphicsOutput>()
            .expect_success("Failed to retrieve graphics output")
            .get())
    };
    let frame_buffer_ptr = gop.frame_buffer().as_mut_ptr();
    let frame_buffer_size = gop.frame_buffer().size();

    let frame_buffer = rumikan_shared::graphics::FrameBuffer::new(
        frame_buffer_ptr,
        gop.current_mode_info().resolution().0,
        gop.current_mode_info().resolution().1,
        gop.current_mode_info().stride(),
        (match gop.current_mode_info().pixel_format() {
            PixelFormat::Rgb => Some(rumikan_shared::graphics::PixelFormat::Rgb),
            PixelFormat::Bgr => Some(rumikan_shared::graphics::PixelFormat::Bgr),
            _ => None,
        }).expect("Unsupported pixel format")
    );

    info!("Resolution: {}x{}, Pixel Format: {:?}, {} pixels/line",
          frame_buffer.resolution().0,
          frame_buffer.resolution().1,
          frame_buffer.pixel_format(),
          frame_buffer.stride());

    info!("Frame buffer addr: {:p}, size: 0x{:x}", frame_buffer_ptr, frame_buffer_size);

    frame_buffer
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

/// Header for generic file information
/// This struct is originally defined in uefi-rs crate, but it's not exposed.
/// Redefined here to retrieve the size of this struct to allocate buffer for FileInfo.
#[derive(Debug)]
#[repr(C)]
struct FileInfoHeader {
    size: u64,
    file_size: u64,
    physical_size: u64,
    create_time: Time,
    last_access_time: Time,
    modification_time: Time,
    attribute: FileAttribute,
}
