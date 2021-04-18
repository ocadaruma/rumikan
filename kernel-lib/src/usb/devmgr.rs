use crate::usb::classdriver::ClassDriver;
use crate::usb::endpoint::EndpointId;
use crate::usb::mem::allocate;
use crate::usb::ring::{SetupData, SetupStageTrb, TransferEventTrb, Trb, TrbType};
use crate::util::ArrayMap;
use core::mem::size_of;
use core::ptr::{null, null_mut};
use xhci::context::byte32::Device as DeviceContext;

#[derive(Debug)]
pub enum Error {
    AllocError(crate::usb::mem::Error),
    TransferFailed,
    NoWaiter,
    NoCorrespondingSetupStage,
    NotImplemented,
    ClassDriverError(crate::usb::classdriver::Error),
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
    setup_stage_map: ArrayMap<*const Trb, *const SetupStageTrb, 16>,
    event_waiters: ArrayMap<SetupData, ClassDriver, 4>,
    is_initialized: bool,
    initialize_phase: u8,
}

impl UsbDevice {
    pub fn on_transfer_event_received(&mut self, trb: &TransferEventTrb) -> Result<()> {
        let residual_length = trb.transfer_length();

        if !(trb.completion_code() == 1 || trb.completion_code() == 13) {
            return Err(Error::TransferFailed);
        }

        if let TrbType::Normal(normal_trb) = unsafe { *trb.trb_pointer() }.specialize() {
            let transfer_length = normal_trb.transfer_length() - residual_length;
            return self.on_interrupt_completed(
                trb.endpoint_id(),
                normal_trb.pointer(),
                transfer_length,
            );
        }

        let setup_stage_trb = if let Some(trb) = self.setup_stage_map.remove(&trb.trb_pointer()) {
            trb
        } else {
            return Err(Error::NoCorrespondingSetupStage);
        };

        let setup_data = SetupData::from_trb(unsafe { setup_stage_trb.read() });
        match unsafe { *trb.trb_pointer() }.specialize() {
            TrbType::DataStage(data_stage_trb) => {
                let transfer_length = data_stage_trb.trb_transfer_length() - residual_length;
                self.on_control_completed(
                    trb.endpoint_id(),
                    setup_data,
                    data_stage_trb.data_buffer_pointer(),
                    transfer_length,
                )
            }
            TrbType::StatusStage(_) => {
                self.on_control_completed(trb.endpoint_id(), setup_data, null(), 0)
            }
            _ => Err(Error::NotImplemented),
        }
    }

    fn on_interrupt_completed(
        &self,
        endpoint_id: EndpointId,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        if let Some(driver) = self.class_drivers[endpoint_id.number() as usize] {
            return driver
                .on_interrupt_completed(endpoint_id, buf, len)
                .map_err(Error::ClassDriverError);
        } else {
            Err(Error::NoWaiter)
        }
    }

    fn on_control_completed(
        &mut self,
        endpoint_id: EndpointId,
        setup_data: SetupData,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        if self.is_initialized {
            if let Some(waiter) = self.event_waiters.get(&setup_data) {
                return waiter
                    .on_control_completed(endpoint_id, setup_data, buf, len)
                    .map_err(Error::ClassDriverError);
            }
            return Err(Error::NoWaiter);
        }
        unimplemented!()
    }
}
