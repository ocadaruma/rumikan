use crate::usb::descriptor::InterfaceDescriptor;
use crate::usb::endpoint::{EndpointConfig, EndpointId, EndpointType};
use crate::usb::mem::allocate;
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
                unsafe {
                    driver_ptr.write(HidMouseDriver {
                        num_observers: 0,
                        interface_index: desc.interface_number(),
                        endpoint_interrupt_in: EndpointId::new(0),
                        buf: allocate(1024, None, None)
                            .expect("Failed to allocate memory for driver"),
                    });
                }
                return Some(ClassDriver::HidMouse(driver_ptr));
            }
        }
        None
    }

    pub fn on_interrupt_completed(&self, ep_id: EndpointId, len: u32) -> Result<()> {
        match self {
            ClassDriver::HidMouse(driver) => {
                unsafe { driver.read() }.on_interrupt_completed(ep_id, len)
            }
        }
        Ok(())
    }

    pub fn set_endpoint(&mut self, config: &EndpointConfig) {
        match self {
            ClassDriver::HidMouse(driver) => unsafe { driver.read() }.set_endpoint(config),
        }
    }

    pub fn interface_index(&self) -> u8 {
        match self {
            ClassDriver::HidMouse(driver) => unsafe { driver.read() }.interface_index,
        }
    }

    pub fn buffer(&self) -> *const () {
        match self {
            ClassDriver::HidMouse(driver) => unsafe { driver.read() }.buf,
        }
    }

    pub fn in_packet_size(&self) -> usize {
        match self {
            ClassDriver::HidMouse(_) => HidMouseDriver::IN_PACKET_SIZE,
        }
    }

    pub fn endpoint_interrupt_in(&self) -> EndpointId {
        match self {
            ClassDriver::HidMouse(driver) => unsafe { driver.read() }.endpoint_interrupt_in,
        }
    }
}

#[derive(Debug)]
pub struct HidMouseDriver {
    num_observers: usize,
    interface_index: u8,
    endpoint_interrupt_in: EndpointId,
    buf: *const (),
}

impl HidMouseDriver {
    const IN_PACKET_SIZE: usize = 3;

    pub fn set_endpoint(&mut self, config: &EndpointConfig) {
        if config.endpoint_type == EndpointType::Interrupt && config.endpoint_id.is_in() {
            self.endpoint_interrupt_in = config.endpoint_id;
        }
    }

    pub fn on_interrupt_completed(&self, ep_id: EndpointId, _len: u32) {
        if ep_id.is_in() {
            let (x, y) = unsafe {
                let ptr = self.buf as *const u8;
                (ptr.add(1).read(), ptr.add(2).read())
            };
            info!("event received. (x, y) = ({}, {})", x, y);
        }
        // todo!()
    }
}
