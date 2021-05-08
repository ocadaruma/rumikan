use crate::usb::endpoint::EndpointId;
use crate::usb::port::PortSpeed;
use bit_field::{BitArray, BitField};
use core::convert::TryFrom;

#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SlotContext {
    data: [u32; 8],
}
impl SlotContext {
    getbits!(pub root_hub_port_num: u8; data[1]; 16; 8);
    setbits!(pub set_root_hub_port_num: u8; data[1]; 16; 8);
    setbits!(pub set_route_string: u32; data[0]; 0; 20);
    getbits!(_speed: u8; data[0]; 20; 4);
    setbits!(_set_speed: u8; data[0]; 20; 4);
    setbits!(pub set_context_entries: u8; data[0]; 27; 5);

    pub fn speed(&self) -> Result<PortSpeed, ()> {
        PortSpeed::try_from(self._speed())
    }

    pub fn set_speed(&mut self, s: PortSpeed) {
        self._set_speed(s as u8);
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct EndpointContext {
    data: [u64; 4],
}
impl EndpointContext {
    setbits!(pub set_mult: u8; data; 8; 2);
    setbits!(pub set_max_primary_streams: u8; data; 10; 5);
    setbits!(pub set_interval: u8; data; 16; 8);
    setbits!(pub set_error_count: u8; data; 33; 2);
    setbits!(pub set_endpoint_type: u8; data; 35; 3);
    setbits!(pub set_max_burst_size: u8; data; 40; 8);
    setbits!(pub set_max_packet_size: u16; data; 48; 16);
    setbit!(pub set_dequeue_cycle_state; data; 64);
    setbits!(_set_transfer_ring_buffer: u64; data; 68; 60);
    setbits!(pub set_average_trb_length: u16; data; 128; 16);

    pub fn set_transfer_ring_buffer(&mut self, ptr: u64) {
        self._set_transfer_ring_buffer(ptr >> 4);
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct DeviceContext {
    pub slot_context: SlotContext,
    pub endpoint_contexts: [EndpointContext; 31],
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct InputControlContext {
    _drop_context_flags: u32,
    add_context_flags: u32,
    _reserved1: [u32; 5],
    _configuration_value: u8,
    _interface_number: u8,
    _alternate_settings: u8,
    _reserved2: u8,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct InputContext {
    pub input_control_context: InputControlContext,
    pub slot_context: SlotContext,
    endpoint_contexts: [EndpointContext; 31],
}
impl InputContext {
    pub fn enable_slot_context(&mut self) -> &mut SlotContext {
        self.input_control_context.add_context_flags |= 1;
        &mut self.slot_context
    }

    pub fn enable_endpoint(&mut self, dci: EndpointId) -> &mut EndpointContext {
        self.input_control_context.add_context_flags |= 1 << dci.address();
        &mut self.endpoint_contexts[dci.address() as usize - 1]
    }
}
