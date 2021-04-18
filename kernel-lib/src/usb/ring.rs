use crate::usb::endpoint::EndpointId;
use crate::usb::mem::allocate_array;
use crate::usb::ring::TrbType::Unsupported;
use crate::usb::IdentityMapper;
use bit_field::BitField;
use core::mem::transmute;
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

impl Trb {
    pub fn cycle_bit(&self) -> bool {
        self.0.get_bit(65)
    }

    pub fn specialize(&self) -> TrbType {
        let n = self.0.get_bits(106..112) as u8;
        match n {
            NormalTrb::TYPE => TrbType::Normal(NormalTrb(self.0)),
            TransferEventTrb::TYPE => TrbType::TransferEvent(TransferEventTrb(self.0)),
            CommandCompletionEventTrb::TYPE => {
                TrbType::CommandCompletionEvent(CommandCompletionEventTrb(self.0))
            }
            PortStatusChangeEventTrb::TYPE => {
                TrbType::PortStatusChangeEvent(PortStatusChangeEventTrb(self.0))
            }
            DataStageTrb::TYPE => TrbType::DataStage(DataStageTrb(self.0)),
            StatusStageTrb::TYPE => TrbType::StatusStage(StatusStageTrb(self.0)),
            _ => Unsupported,
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct TransferEventTrb(u128);
impl TransferEventTrb {
    pub const TYPE: u8 = 32;

    pub fn slot_id(&self) -> u8 {
        self.0.get_bits(120..128) as u8
    }

    pub fn transfer_length(&self) -> u32 {
        self.0.get_bits(64..88) as u32
    }

    pub fn completion_code(&self) -> u8 {
        self.0.get_bits(88..96) as u8
    }

    pub fn trb_pointer(&self) -> *const Trb {
        unsafe { transmute(self.0.get_bits(0..64) as u64) }
    }

    pub fn endpoint_id(&self) -> EndpointId {
        EndpointId::new(self.0.get_bits(112..117) as u32)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CommandCompletionEventTrb(u128);
impl CommandCompletionEventTrb {
    pub const TYPE: u8 = 33;
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PortStatusChangeEventTrb(u128);
impl PortStatusChangeEventTrb {
    pub const TYPE: u8 = 34;

    pub fn port_id(&self) -> u8 {
        self.0.get_bits(24..32) as u8
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct EnableSlotCommandTrb(u128);
impl EnableSlotCommandTrb {
    pub const TYPE: u8 = 9;

    pub fn new() -> EnableSlotCommandTrb {
        let mut bits = 0u128;
        bits.set_bits(106..112, 9);
        EnableSlotCommandTrb(bits)
    }

    pub fn data(&self) -> u128 {
        self.0
    }
}

impl Default for EnableSlotCommandTrb {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct LinkTrb(u128);
impl LinkTrb {
    pub const TYPE: u8 = 6;

    pub fn new(ring_segment_pointer: u64) -> LinkTrb {
        let mut bits = 0u128;
        bits.set_bits(106..112, 6);
        bits.set_bits(4..64, ring_segment_pointer as u128);

        LinkTrb(bits)
    }

    pub fn set_toggle_cycle(&mut self, b: bool) {
        self.0.set_bit(97, b);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct NormalTrb(u128);
impl NormalTrb {
    pub const TYPE: u8 = 1;

    pub fn transfer_length(&self) -> u32 {
        self.0.get_bits(64..81) as u32
    }

    pub fn pointer(&self) -> *const () {
        unsafe { transmute(self.0.get_bits(0..64) as u64) }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct SetupStageTrb(u128);
impl SetupStageTrb {
    pub const TYPE: u8 = 2;

    pub fn request_type(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }

    pub fn request(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn value(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }

    pub fn index(&self) -> u16 {
        self.0.get_bits(32..48) as u16
    }

    pub fn length(&self) -> u16 {
        self.0.get_bits(48..64) as u16
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct DataStageTrb(u128);
impl DataStageTrb {
    pub const TYPE: u8 = 3;

    pub fn data_buffer_pointer(&self) -> *const () {
        unsafe { transmute(self.0.get_bits(0..64) as u64) }
    }

    pub fn trb_transfer_length(&self) -> u32 {
        self.0.get_bits(64..81) as u32
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct StatusStageTrb(u128);
impl StatusStageTrb {
    pub const TYPE: u8 = 4;
}

#[derive(Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct SetupData(u64);
impl SetupData {
    pub fn from_trb(setup_stage_trb: SetupStageTrb) -> SetupData {
        let mut data = 0u64;

        data.set_bits(0..8, setup_stage_trb.request_type() as u64);
        data.set_bits(8..16, setup_stage_trb.request() as u64);
        data.set_bits(16..32, setup_stage_trb.value() as u64);
        data.set_bits(32..48, setup_stage_trb.index() as u64);
        data.set_bits(48..64, setup_stage_trb.length() as u64);

        SetupData(data)
    }

    pub fn request(&self) -> Request {
        match self.0.get_bits(8..16) as u8 {
            6 => Request::GetDescriptor,
            9 => Request::SetConfiguration,
            _ => Request::Unsupported,
        }
    }

    pub fn value(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Request {
    Unsupported,
    GetDescriptor,
    SetConfiguration,
}

#[derive(Debug)]
pub enum TrbType {
    Unsupported,
    Normal(NormalTrb),
    TransferEvent(TransferEventTrb),
    CommandCompletionEvent(CommandCompletionEventTrb),
    PortStatusChangeEvent(PortStatusChangeEventTrb),
    DataStage(DataStageTrb),
    StatusStage(StatusStageTrb),
}

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

    pub fn push(&mut self, data: u128) {
        let ptr = unsafe { self.buffer.add(self.write_index) };
        self.copy_to_last(data);

        self.write_index += 1;
        if self.write_index == self.len - 1 {
            let mut link = LinkTrb::new(self.buffer as u64);
            link.set_toggle_cycle(true);
            self.copy_to_last(link.0);

            self.write_index = 0;
            self.cycle_bit = !self.cycle_bit;
        }
    }

    fn copy_to_last(&mut self, data: u128) {
        let mut msb32 = data.get_bits(96..128) as u32;
        msb32 = (msb32 & 0xfffffffe) | (self.cycle_bit as u32);

        let mut data = data;
        data.set_bits(96..128, msb32 as u128);
        unsafe {
            self.buffer.add(self.write_index).write(Trb(data));
        }
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

    pub fn ring_segment_base_address(&self) -> u64 {
        self.0.get_bits(0..64) as u64
    }

    pub fn ring_segment_size(&self) -> u16 {
        self.0.get_bits(64..80) as u16
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

    pub fn peek_front(&self) -> Option<Trb> {
        let ptr: *const Trb =
            unsafe { transmute(self.interrupter.read().erdp.event_ring_dequeue_pointer()) };
        let trb = unsafe { *ptr };

        if trb.cycle_bit() == self.cycle_bit {
            Some(trb)
        } else {
            None
        }
    }

    pub fn pop(&mut self) {
        let ptr: *const Trb =
            unsafe { transmute(self.interrupter.read().erdp.event_ring_dequeue_pointer()) };
        let mut ptr = unsafe { ptr.add(1) };

        let segment_begin: *const Trb =
            unsafe { transmute((*self.segment_table).ring_segment_base_address()) };
        let segment_end =
            unsafe { segment_begin.add((*self.segment_table).ring_segment_size() as usize) };

        if ptr == segment_end {
            ptr = segment_begin;
            self.cycle_bit = !self.cycle_bit;
        }

        self.interrupter.update(|i| {
            i.erdp.set_event_ring_dequeue_pointer(ptr as u64);
        });
    }
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bit_field::BitField;

    #[test]
    fn test_bit_field() {
        let x = 0x00112000;
        assert_eq!(x.get_bits(16..32) as u16, 17);
    }
}
