use crate::usb::descriptor::InterfaceDescriptor;
use crate::usb::endpoint::{EndpointConfig, EndpointId};
use crate::usb::mem::allocate;
use crate::usb::ring::SetupData;
use core::mem::size_of;

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
    pub fn new(desc: &InterfaceDescriptor) -> Option<Self> {
        if desc.interface_class() == 3 && desc.interface_sub_class() == 1 {
            if desc.interface_protocol() == 1 {
                return None;
            } else if desc.interface_protocol() == 2 {
                let driver_ptr: *mut HidMouseDriver =
                    allocate(size_of::<HidMouseDriver>(), None, None)
                        .expect("Failed to allocate memory for driver");
                return Some(ClassDriver::HidMouse(driver_ptr));
            }
        }
        None
    }

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
        Ok(())
    }

    pub fn on_control_completed(
        &self,
        ep_id: EndpointId,
        setup_data: SetupData,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        // todo!()
        Ok(())
    }

    pub fn set_endpoint(&mut self, config: &EndpointConfig) {
        // todo!()
    }
}

#[derive(Debug)]
pub struct HidMouseDriver {
    num_observers: usize,
    interface_index: u8,
}

impl HidMouseDriver {
    pub fn on_interrupt_completed(&self, ep_id: EndpointId, buf: *const (), len: u32) {
        if ep_id.is_in() {
            let (x, y) = unsafe {
                let ptr = buf as *const u8;
                (ptr.add(1).read(), ptr.add(2).read())
            };
            printk!("event received. (x, y) = ({}, {})\n", x, y);
        }
        // todo!()
    }
}
