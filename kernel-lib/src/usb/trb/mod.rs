pub mod ring;

use crate::error::ErrorContext;
use crate::usb::endpoint::EndpointId;
use crate::usb::SlotId;
use bit_field::BitField;

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum ErrorType {
    AllocError(crate::usb::mem::Error),
}

/// A trait must be implemented by all TRB structs
pub trait Trb {
    const TYPE: u8;

    fn generalize(&self) -> &GenericTrb {
        let ptr: *const Self = self;
        unsafe { &*(ptr as *const GenericTrb) }
    }

    fn generalize_mut(&mut self) -> &mut GenericTrb {
        let ptr: *mut Self = self;
        unsafe { &mut *(ptr as *mut GenericTrb) }
    }
}

#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone)]
pub struct GenericTrb {
    data: u128,
}

impl GenericTrb {
    getbit!(pub cycle_bit; data; 96);
    setbit!(pub set_cycle_bit; data; 96);
    getbits!(pub trb_type: u8; data; 106; 6);

    pub fn specialize<T: Trb>(&self) -> Option<&T> {
        if self.trb_type() == T::TYPE {
            Some(unsafe { &*(self as *const Self as *const T) })
        } else {
            None
        }
    }

    pub fn data(&self) -> u128 {
        self.data
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct NormalTrb {
    data: u128,
}

impl Trb for NormalTrb {
    const TYPE: u8 = 1;
}

impl NormalTrb {
    withbits!(pub with_pointer: u64; data; 0; 64);
    getbits!(pub transfer_length: u32; data; 64; 17);
    withbits!(pub with_transfer_length: u32; data; 64; 17);
    withbit!(pub with_interrupt_on_short_packet; data; 98);
    withbit!(pub with_interrupt_on_completion; data; 101);
    setbits!(set_trb_type: u8; data; 106; 6);

    pub fn new() -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct SetupStageTrb {
    data: u128,
}

impl Trb for SetupStageTrb {
    const TYPE: u8 = 2;
}

impl SetupStageTrb {
    pub const TRANSFER_TYPE_NO_DATA_STAGE: u8 = 0;
    pub const TRANSFER_TYPE_OUT_DATA_STAGE: u8 = 2;
    pub const TRANSFER_TYPE_IN_DATA_STAGE: u8 = 3;

    getbits!(request_type: u8; data; 0; 8);
    setbits!(set_request_type: u8; data; 0; 8);
    getbits!(request: u8; data; 8; 8);
    setbits!(set_request: u8; data; 8; 8);
    getbits!(value: u16; data; 16; 16);
    setbits!(set_value: u16; data; 16; 16);
    getbits!(index: u16; data; 32; 16);
    setbits!(set_index: u16; data; 32; 16);
    getbits!(length: u16; data; 48; 16);
    setbits!(set_length: u16; data; 48; 16);
    setbits!(set_trb_transfer_length: u32; data; 64; 17);
    setbit!(set_immediate_data; data; 102);
    setbits!(set_trb_type: u8; data; 106; 6);
    setbits!(set_transfer_type: u8; data; 112; 2);

    pub fn new() -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb
    }

    pub fn setup_data(&self) -> SetupData {
        SetupData::new()
            .with_request_type(RequestType {
                data: self.request_type(),
            })
            .with_request(self.request())
            .with_value(self.value())
            .with_index(self.index())
            .with_length(self.length())
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct DataStageTrb {
    data: u128,
}

impl Trb for DataStageTrb {
    const TYPE: u8 = 3;
}

impl DataStageTrb {
    getbits!(pub data_buffer_pointer: u64; data; 0; 64);
    withbits!(pub with_data_buffer_pointer: u64; data; 0; 64);
    getbits!(pub trb_transfer_length: u32; data; 64; 17);
    withbits!(pub with_trb_transfer_length: u32; data; 64; 17);
    withbit!(pub with_interrupt_on_completion; data; 101);
    setbits!(set_trb_type: u8; data; 106; 6);
    withbit!(pub with_direction_in; data; 112);

    pub fn new() -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct StatusStageTrb {
    data: u128,
}

impl Trb for StatusStageTrb {
    const TYPE: u8 = 4;
}

impl StatusStageTrb {
    withbit!(pub with_interrupt_on_completion; data; 101);
    setbits!(set_trb_type: u8; data; 106; 6);
    withbit!(pub with_direction_in; data; 112);

    pub fn new() -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct LinkTrb {
    data: u128,
}

impl Trb for LinkTrb {
    const TYPE: u8 = 6;
}

impl LinkTrb {
    setbits!(set_ring_segment_pointer: u64; data; 4; 60);
    setbit!(set_toggle_cycle; data; 97);
    setbits!(set_trb_type: u8; data; 106; 6);

    pub fn new(ring_segment_pointer: u64) -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb.set_ring_segment_pointer(ring_segment_pointer >> 4);
        trb.set_toggle_cycle(true);
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct EnableSlotCommandTrb {
    data: u128,
}

impl Trb for EnableSlotCommandTrb {
    const TYPE: u8 = 9;
}

impl EnableSlotCommandTrb {
    setbits!(set_trb_type: u8; data; 106; 6);

    pub fn new() -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct AddressDeviceCommandTrb {
    data: u128,
}

impl Trb for AddressDeviceCommandTrb {
    const TYPE: u8 = 11;
}

impl AddressDeviceCommandTrb {
    setbits!(set_input_context_ptr: u64; data; 4; 60);
    setbits!(set_trb_type: u8; data; 106; 6);
    setbits!(set_slot_id: u8; data; 120; 8);

    pub fn new(slot_id: SlotId, input_context_ptr: u64) -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb.set_input_context_ptr(input_context_ptr >> 4);
        trb.set_slot_id(slot_id.value());
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct ConfigureEndpointCommandTrb {
    data: u128,
}

impl Trb for ConfigureEndpointCommandTrb {
    const TYPE: u8 = 12;
}

impl ConfigureEndpointCommandTrb {
    setbits!(set_input_context_ptr: u64; data; 4; 60);
    setbits!(set_trb_type: u8; data; 106; 6);
    setbits!(set_slot_id: u8; data; 120; 8);

    pub fn new(slot_id: SlotId, input_context_ptr: u64) -> Self {
        let mut trb = Self { data: 0 };
        trb.set_trb_type(Self::TYPE);
        trb.set_input_context_ptr(input_context_ptr >> 4);
        trb.set_slot_id(slot_id.value());
        trb
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct TransferEventTrb {
    data: u128,
}

impl Trb for TransferEventTrb {
    const TYPE: u8 = 32;
}

impl TransferEventTrb {
    getbits!(pub issuer_pointer: u64; data; 0; 64);
    getbits!(pub transfer_length: u32; data; 64; 24);
    getbits!(pub completion_code: u8; data; 88; 8);
    getbits!(_endpoint_id: u8; data; 112; 5);
    getbits!(_slot_id: u8; data; 120; 8);

    pub fn slot_id(&self) -> SlotId {
        SlotId::new(self._slot_id())
    }

    pub fn issuer_trb(&self) -> &GenericTrb {
        unsafe { &*(self.issuer_pointer() as *const GenericTrb) }
    }

    pub fn endpoint_id(&self) -> EndpointId {
        EndpointId::new(self._endpoint_id())
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct CommandCompletionEventTrb {
    data: u128,
}

impl Trb for CommandCompletionEventTrb {
    const TYPE: u8 = 33;
}

impl CommandCompletionEventTrb {
    getbits!(_trb_pointer: u64; data; 4; 60);
    getbits!(_slot_id: u8; data; 120; 8);

    pub fn issuer(&self) -> &GenericTrb {
        unsafe { &*((self._trb_pointer() << 4) as *const GenericTrb) }
    }

    pub fn slot_id(&self) -> SlotId {
        SlotId::new(self._slot_id())
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct PortStatusChangeEventTrb {
    data: u128,
}

impl Trb for PortStatusChangeEventTrb {
    const TYPE: u8 = 34;
}

impl PortStatusChangeEventTrb {
    getbits!(pub port_id: u8; data; 24; 8);
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(transparent)]
pub struct SetupData {
    data: u64,
}

impl SetupData {
    pub const REQUEST_GET_DESCRIPTOR: u8 = 6;
    pub const REQUEST_SET_CONFIGURATION: u8 = 9;
    pub const REQUEST_SET_PROTOCOL: u8 = 11;

    getbits!(request_type: u8; data; 0; 8);
    withbits!(_with_request_type: u8; data; 0; 8);
    getbits!(pub request: u8; data; 8; 8);
    withbits!(pub with_request: u8; data; 8; 8);
    getbits!(value: u16; data; 16; 16);
    withbits!(pub with_value: u16; data; 16; 16);
    getbits!(index: u16; data; 32; 16);
    withbits!(pub with_index: u16; data; 32; 16);
    getbits!(length: u16; data; 48; 16);
    withbits!(pub with_length: u16; data; 48; 16);

    pub fn new() -> Self {
        Self { data: 0 }
    }

    pub fn trb(&self, transfer_type: u8) -> SetupStageTrb {
        let mut trb = SetupStageTrb::new();

        trb.set_trb_transfer_length(8);
        trb.set_immediate_data(true);
        trb.set_transfer_type(transfer_type);
        trb.set_request_type(self.request_type());
        trb.set_request(self.request());
        trb.set_value(self.value());
        trb.set_index(self.index());
        trb.set_length(self.length());
        trb
    }

    pub fn with_request_type(self, request_type: RequestType) -> Self {
        self._with_request_type(request_type.data)
    }
}

#[repr(transparent)]
#[derive(Debug, Eq, PartialEq)]
pub struct RequestType {
    data: u8,
}

impl RequestType {
    pub const TYPE_STANDARD: u8 = 0;
    pub const TYPE_CLASS: u8 = 1;

    pub const RECIPIENT_DEVICE: u8 = 0;
    pub const RECIPIENT_INTERFACE: u8 = 1;

    pub const DIRECTION_HOST_TO_DEVICE: bool = false;
    pub const DIRECTION_DEVICE_TO_HOST: bool = true;

    withbits!(pub with_recipient: u8; data; 0; 5);
    withbits!(pub with_type: u8; data; 5; 2);
    withbit!(pub with_direction; data; 7);

    pub fn new() -> Self {
        Self { data: 0 }
    }
}
