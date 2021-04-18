use crate::usb::endpoint::EndpointId;

#[derive(Debug, Copy, Clone)]
pub enum ClassDriver {
    HidMouse(*const HidMouseDriver),
}

impl ClassDriver {
    pub fn on_interrupt_completed(&self, ep_id: EndpointId, buf: *const (), len: u32) {
        match self {
            ClassDriver::HidMouse(driver) => {
                unsafe { driver.read() }.on_interrupt_completed(ep_id, buf, len)
            }
        }
        unimplemented!()
    }
}

pub struct HidMouseDriver {
    num_observers: usize,
}

impl HidMouseDriver {
    pub fn on_interrupt_completed(&self, ep_id: EndpointId, buf: *const (), len: u32) {
        unimplemented!()
    }
}
