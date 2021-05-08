use super::{ErrorType, Result};
use crate::usb::mem::allocate_array;
use crate::usb::trb::{GenericTrb, LinkTrb, Trb};
use crate::usb::xhci::{Accessor, InterrupterRegisterSet};
use bit_field::BitField;
use core::ptr::null_mut;

#[derive(Debug)]
pub struct PushResult<T> {
    pub ptr: u64,
    pub trb: T,
}

#[derive(Debug)]
pub struct Ring {
    buffer: *mut GenericTrb,
    len: usize,
    cycle_bit: bool,
    write_index: usize,
}

impl Ring {
    pub fn new() -> Self {
        Self {
            buffer: null_mut(),
            len: 0,
            cycle_bit: false,
            write_index: 0,
        }
    }

    pub fn initialize(&mut self, len: usize) -> Result<()> {
        self.cycle_bit = true;
        self.write_index = 0;
        self.len = len;

        self.buffer = allocate_array::<GenericTrb>(len, Some(64), Some(64 * 1024))
            .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;
        Ok(())
    }

    pub fn buffer_pointer(&self) -> u64 {
        self.buffer as u64
    }

    /// Push the TRB to the ring, return the trb with
    /// the pointer to the trb in the buffer
    pub fn push<T: Trb>(&mut self, mut trb: T) -> PushResult<T> {
        let ptr = unsafe { self.buffer.add(self.write_index) };
        self.copy_to_last(trb.generalize_mut());

        self.write_index += 1;
        if self.write_index == self.len - 1 {
            let mut link = LinkTrb::new(self.buffer as u64);
            self.copy_to_last(link.generalize_mut());

            self.write_index = 0;
            self.cycle_bit = !self.cycle_bit;
        }

        PushResult {
            trb,
            ptr: ptr as u64,
        }
    }

    fn copy_to_last(&mut self, trb: &mut GenericTrb) {
        trb.set_cycle_bit(self.cycle_bit);

        // write lower 96 bits first, then write higher 32 bits in single instruction which
        // includes cycle_bit next to prevent the TRB is dequeued by  xHC unexpectedly early
        for i in 0..3 {
            unsafe {
                (self.buffer.add(self.write_index) as *mut u32)
                    .add(i)
                    .write_volatile(trb.data().get_bits((i * 32)..(i * 32 + 32)) as u32);
            }
        }
        unsafe {
            (self.buffer.add(self.write_index) as *mut u32)
                .add(3)
                .write_volatile(trb.data.get_bits(96..128) as u32);
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct EventRingSegmentTableEntry {
    data: u128,
}

impl EventRingSegmentTableEntry {
    getbits!(pub ring_segment_base_address: u64; data; 0; 64);
    setbits!(pub set_ring_segment_base_address: u64; data; 0; 64);
    getbits!(pub ring_segment_size: u16; data; 64; 16);
    setbits!(pub set_ring_segment_size: u16; data; 64; 16);
}

#[derive(Debug)]
pub struct EventRing {
    buffer: *mut GenericTrb,
    segment_table: *mut EventRingSegmentTableEntry,
    interrupter: Accessor<InterrupterRegisterSet>,
    len: usize,
    cycle_bit: bool,
}

impl EventRing {
    pub fn new() -> Self {
        Self {
            buffer: null_mut(),
            segment_table: null_mut(),
            interrupter: Accessor::null(),
            len: 0,
            cycle_bit: false,
        }
    }

    pub fn initialize(
        &mut self,
        len: usize,
        interrupter: Accessor<InterrupterRegisterSet>,
    ) -> Result<()> {
        self.interrupter = interrupter;
        self.cycle_bit = true;
        self.len = len;

        self.buffer = allocate_array::<GenericTrb>(len, Some(64), Some(64 * 1024))
            .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;

        self.segment_table =
            allocate_array::<EventRingSegmentTableEntry>(1, Some(64), Some(64 * 1024))
                .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;
        let mut table_entry = EventRingSegmentTableEntry::default();
        table_entry.set_ring_segment_size(len as u16);
        table_entry.set_ring_segment_base_address(self.buffer as u64);

        unsafe {
            self.segment_table.write_volatile(table_entry);
        }

        let buffer_ptr = self.buffer as u64;
        let segment_table_ptr = self.segment_table as u64;

        let reg = self.interrupter.as_mut();
        reg.erstsz
            .update(|r| r.set_event_ring_segment_table_size(1));
        reg.erdp
            .update(|r| r.set_event_ring_dequeue_pointer(buffer_ptr));
        reg.erstba
            .update(|r| r.set_event_ring_segment_table_base_address(segment_table_ptr));

        Ok(())
    }

    pub fn poll(&mut self) -> Option<GenericTrb> {
        let trb = unsafe {
            &*(self
                .interrupter
                .as_ref()
                .erdp
                .read()
                .event_ring_dequeue_pointer() as *const GenericTrb)
        };

        if trb.cycle_bit() == self.cycle_bit {
            self.pop();
            Some(*trb)
        } else {
            None
        }
    }

    fn pop(&mut self) {
        let mut ptr = unsafe {
            (self
                .interrupter
                .as_ref()
                .erdp
                .read()
                .event_ring_dequeue_pointer() as *const GenericTrb)
                .add(1)
        };

        let segment_begin: *const GenericTrb =
            unsafe { (self.segment_table.read()).ring_segment_base_address() as *const GenericTrb };
        let segment_end =
            unsafe { segment_begin.add((self.segment_table.read()).ring_segment_size() as usize) };

        if ptr == segment_end {
            ptr = segment_begin;
            self.cycle_bit = !self.cycle_bit;
        }
        self.interrupter.as_mut().erdp.update(|r| {
            r.set_event_ring_dequeue_pointer(ptr as u64);
        });
    }
}
