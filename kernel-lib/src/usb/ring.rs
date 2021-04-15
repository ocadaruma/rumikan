use crate::usb::mem::allocate_array;
use crate::usb::IdentityMapper;
use bit_field::BitField;
use xhci::accessor;
use xhci::registers::InterruptRegisterSet;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    AllocError(crate::usb::mem::Error),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct Trb(u128);

/// Struct that represents command ring.
#[derive(Debug)]
pub struct Ring {
    buffer: *mut Trb,
    len: usize,
    cycle_bit: bool,
    write_index: usize,
}

impl Ring {
    pub fn new() -> Ring {
        Ring {
            buffer: core::ptr::null_mut(),
            len: 0,
            cycle_bit: false,
            write_index: 0,
        }
    }

    pub fn initialize(&mut self, len: usize) -> Result<()> {
        self.cycle_bit = true;
        self.write_index = 0;
        self.len = len;

        match allocate_array::<Trb>(len, Some(64), Some(64 * 1024)) {
            Ok(ptr) => {
                self.buffer = ptr;
                Ok(())
            }
            Err(err) => Err(Error::AllocError(err)),
        }
    }

    pub fn ptr_as_u64(&self) -> u64 {
        self.buffer as u64
    }
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct EventRingSegmentTableEntry(u128);

impl EventRingSegmentTableEntry {
    pub fn set_ring_segment_base_address(&mut self, addr: u64) {
        self.0.set_bits(0..64, addr as u128);
    }

    pub fn set_ring_segment_size(&mut self, size: u16) {
        self.0.set_bits(64..80, size as u128);
    }
}

#[derive(Debug)]
pub struct EventRing {
    buffer: *mut Trb,
    segment_table: *mut EventRingSegmentTableEntry,
    interrupter: accessor::Single<InterruptRegisterSet, IdentityMapper>,
    len: usize,
    cycle_bit: bool,
}

impl EventRing {
    pub fn new() -> EventRing {
        EventRing {
            buffer: core::ptr::null_mut(),
            segment_table: core::ptr::null_mut(),
            interrupter: unsafe { accessor::Single::new(0, IdentityMapper) },
            len: 0,
            cycle_bit: false,
        }
    }

    pub fn initialize(
        &mut self,
        len: usize,
        mut interrupter: accessor::Single<InterruptRegisterSet, IdentityMapper>,
    ) -> Result<()> {
        self.cycle_bit = true;
        self.len = len;

        let buffer_ptr = match allocate_array::<Trb>(len, Some(64), Some(64 * 1024)) {
            Ok(ptr) => ptr,
            Err(err) => return Err(Error::AllocError(err)),
        };

        let segment_table_ptr =
            match allocate_array::<EventRingSegmentTableEntry>(1, Some(64), Some(64 * 1024)) {
                Ok(ptr) => ptr,
                Err(err) => return Err(Error::AllocError(err)),
            };

        unsafe {
            (*segment_table_ptr).set_ring_segment_size(len as u16);
            (*segment_table_ptr).set_ring_segment_base_address(buffer_ptr as u64);
        }

        interrupter.update(|i| {
            i.erstsz.set(1);
            i.erdp.set_event_ring_dequeue_pointer(buffer_ptr as u64);
            i.erstba.set(segment_table_ptr as u64);
        });
        self.interrupter = interrupter;

        Ok(())
    }
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new()
    }
}
