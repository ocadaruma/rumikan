use crate::usb::endpoint::{EndpointConfig, EndpointId};
use crate::usb::ring::SetupData;

#[derive(Debug)]
pub enum Error {
    NotImplemented,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Copy, Clone)]
pub enum ClassDriver {
    HidMouse(*const HidMouseDriver),
}

impl ClassDriver {
    pub fn on_interrupt_completed(
        &self,
        ep_id: EndpointId,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        match self {
            ClassDriver::HidMouse(driver) => {
                unsafe { driver.read() }.on_interrupt_completed(ep_id, buf, len)
            }
        }
        unimplemented!()
    }

    pub fn on_control_completed(
        &self,
        ep_id: EndpointId,
        setup_data: SetupData,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        unimplemented!()
    }

    pub fn set_endpoint(&mut self, config: &EndpointConfig) {
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
