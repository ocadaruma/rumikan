use crate::usb::descriptor::EndpointDescriptor;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct EndpointNumber(u8);
impl EndpointNumber {
    pub const MAX: u8 = 0x10;
    pub const MAX_ENDPOINT: Self = Self(Self::MAX);

    pub fn new(num: u8) -> Self {
        Self(num)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EndpointId(u8);
impl EndpointId {
    pub const DEFAULT_CONTROL_PIPE_ID: Self = Self(1);

    pub const MAX: u8 = 0x1f;

    pub fn new(addr: u8) -> Self {
        Self(addr)
    }

    pub fn from(ep_num: EndpointNumber, dir_in: bool) -> Self {
        Self(ep_num.0 << 1 | (dir_in as u8))
    }

    pub fn is_in(&self) -> bool {
        (self.0 & 1) == 1
    }

    pub fn number(&self) -> EndpointNumber {
        EndpointNumber::new(self.0 >> 1)
    }

    pub fn address(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub enum EndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

#[derive(Debug, Copy, Clone)]
pub struct EndpointConfig {
    pub endpoint_id: EndpointId,
    pub endpoint_type: EndpointType,
    pub max_packet_size: usize,
    pub interval: u32,
}

impl EndpointConfig {
    pub fn from(desc: &EndpointDescriptor) -> Self {
        Self {
            endpoint_id: EndpointId::from(
                EndpointNumber::new(desc.endpoint_address_number() as u8),
                desc.endpoint_address_dir_in(),
            ),
            endpoint_type: match desc.attributes_transfer_type() {
                0 => EndpointType::Control,
                1 => EndpointType::Isochronous,
                2 => EndpointType::Bulk,
                3 => EndpointType::Interrupt,
                _ => panic!("never"),
            },
            max_packet_size: desc.max_packet_size() as usize,
            interval: desc.interval() as u32,
        }
    }
}
