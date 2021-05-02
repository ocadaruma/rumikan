use crate::usb::endpoint::EndpointId;
use crate::usb::port::PortSpeed;
use bit_field::BitField;
use core::convert::TryFrom;

#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SlotContext([u32; 8]);
impl SlotContext {
    pub fn root_hub_port_num(&self) -> u8 {
        self.0[1].get_bits(16..24) as u8
    }

    pub fn set_root_hub_port_num(&mut self, num: u8) {
        self.0[1].set_bits(16..24, num as u32);
    }

    pub fn set_route_string(&mut self, s: u32) {
        self.0[0].set_bits(0..20, s as u32);
    }

    pub fn speed(&self) -> Result<PortSpeed, ()> {
        PortSpeed::try_from(self.0[0].get_bits(20..24) as u8)
    }

    pub fn set_speed(&mut self, s: PortSpeed) {
        self.0[0].set_bits(20..24, s as u32);
    }

    pub fn set_context_entries(&mut self, entries: u8) {
        self.0[0].set_bits(27..32, entries as u32);
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct EndpointContext {
    bits0: u32,
    bits1: u32,
    bits2: u64,
    bits3: u32,
    _placement: [u32; 3],
}
impl EndpointContext {
    pub fn set_endpoint_type(&mut self, t: u8) {
        self.bits1.set_bits(3..6, t as u32);
    }

    pub fn set_max_packet_size(&mut self, s: u16) {
        self.bits1.set_bits(16..32, s as u32);
    }

    pub fn set_max_burst_size(&mut self, s: u8) {
        self.bits1.set_bits(8..16, s as u32);
    }

    pub fn set_interval(&mut self, i: u8) {
        self.bits0.set_bits(16..24, i as u32);
    }

    pub fn set_average_trb_length(&mut self, l: u16) {
        self.bits3.set_bits(0..16, l as u32);
    }

    pub fn set_dequeue_cycle_state(&mut self, b: bool) {
        self.bits2.set_bit(0, b);
    }

    pub fn set_max_primary_streams(&mut self, s: u8) {
        self.bits0.set_bits(10..15, s as u32);
    }

    pub fn set_mult(&mut self, s: u8) {
        self.bits0.set_bits(8..10, s as u32);
    }

    pub fn set_error_count(&mut self, s: u8) {
        self.bits1.set_bits(1..3, s as u32);
    }

    pub fn set_transfer_ring_buffer(&mut self, ptr: u64) {
        self.bits2.set_bits(4..64, ptr >> 4);
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
    input_control_context: InputControlContext,
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
