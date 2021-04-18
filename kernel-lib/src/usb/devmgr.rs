use crate::usb::classdriver::ClassDriver;
use crate::usb::endpoint::EndpointId;
use crate::usb::mem::allocate;
use crate::usb::ring::{TransferEventTrb, TrbType};
use core::mem::size_of;
use core::ptr::null_mut;
use xhci::context::byte32::Device as DeviceContext;

#[derive(Debug)]
pub enum Error {
    AllocError(crate::usb::mem::Error),
    TransferFailed,
}

pub type Result<T> = core::result::Result<T, Error>;

const NUM_DEVICE_SLOTS: usize = 8;

pub struct DeviceManager {
    max_slots: usize,
    device_context_ptr: *mut DeviceContext,
    devices: *mut UsbDevice,
}

impl DeviceManager {
    pub fn new() -> DeviceManager {
        DeviceManager {
            max_slots: NUM_DEVICE_SLOTS,
            device_context_ptr: null_mut(),
            devices: null_mut(),
        }
    }

    pub fn max_slots(&self) -> usize {
        self.max_slots
    }

    pub fn dcbaa_ptr(&self) -> u64 {
        self.device_context_ptr as u64
    }

    pub fn initialize(&mut self) -> Result<()> {
        let devices_ptr = match allocate::<UsbDevice>(
            size_of::<UsbDevice>() * (self.max_slots + 1),
            None,
            None,
        ) {
            Ok(ptr) => ptr,
            Err(err) => return Err(Error::AllocError(err)),
        };

        let ctx_ptr = match allocate::<DeviceContext>(
            size_of::<DeviceContext>() * (self.max_slots + 1),
            Some(64),
            Some(4096),
        ) {
            Ok(ptr) => ptr,
            Err(err) => return Err(Error::AllocError(err)),
        };
        self.devices = devices_ptr;
        self.device_context_ptr = ctx_ptr;

        Ok(())
    }

    pub fn find_by_slot(&self, slot_id: u8) -> Option<*mut UsbDevice> {
        if slot_id as usize > self.max_slots {
            None
        } else {
            Some(unsafe { self.devices.add(slot_id as usize) })
        }
    }
}

#[derive(Debug)]
pub struct UsbDevice {
    class_drivers: [Option<ClassDriver>; 16],
    buf: [u8; 256],
}

impl UsbDevice {
    pub fn on_transfer_event_received(&self, trb: &TransferEventTrb) -> Result<()> {
        let residual_length = trb.transfer_length();

        if !(trb.completion_code() == 1 || trb.completion_code() == 13) {
            return Err(Error::TransferFailed);
        }

        match unsafe { *trb.trb_pointer() }.specialized() {
            TrbType::Normal(trb) => {
                let transfer_length = trb.transfer_length() - residual_length;
            }
            _ => {}
        }

        unimplemented!()
    }

    fn on_interrupt_completed(&self, endpoint_id: EndpointId, buf: *const (), len: u32) {
        unimplemented!()
    }
}
