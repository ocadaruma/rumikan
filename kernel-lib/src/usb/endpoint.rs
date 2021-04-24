#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EndpointId(u32);

impl EndpointId {
    pub const DEFAULT_CONTROL_PIPE_ID: EndpointId = EndpointId(1);

    pub fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub fn from(ep_num: u32, dir_in: bool) -> Self {
        Self(ep_num << 1 | (dir_in as u32))
    }

    pub fn is_in(&self) -> bool {
        (self.0 & 1) == 1
    }

    pub fn number(&self) -> u32 {
        self.0 >> 1
    }

    pub fn address(&self) -> u32 {
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
