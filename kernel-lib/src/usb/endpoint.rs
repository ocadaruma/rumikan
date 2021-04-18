#[derive(Debug, Copy, Clone)]
pub struct EndpointId(u32);

impl EndpointId {
    pub fn new(addr: u32) -> EndpointId {
        EndpointId(addr)
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
