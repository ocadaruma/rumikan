use crate::usb::classdriver::ClassDriver;
use crate::usb::descriptor::{
    ConfigurationDescriptor, Descriptor, DescriptorType, DeviceDescriptor,
};
use crate::usb::endpoint::{EndpointConfig, EndpointId};
use crate::usb::mem::allocate;
use crate::usb::ring::{
    DataStageTrb, NormalTrb, RequestType, Ring, SetupData, SetupStageTrb, StatusStageTrb,
    TransferEventTrb, Trb, TrbType,
};
use crate::usb::IdentityMapper;
use crate::util::{ArrayMap, ArrayVec};
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
    InvalidPhase,
    InvalidEndpointNumber,
    TransferRingNotSet,
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
    transfer_rings: [Option<Ring>; 31],
    dbreg: xhci::accessor::Single<xhci::registers::doorbell::Register, IdentityMapper>,
    data_buf: *const (),
    ep_configs: ArrayVec<EndpointConfig, 16>,
    setup_stage_map: ArrayMap<*const Trb, *const SetupStageTrb, 16>,
    event_waiters: ArrayMap<SetupData, ClassDriver, 4>,
    is_initialized: bool,
    initialize_phase: u8,
}

impl UsbDevice {
    const DATA_BUF_LEN: u32 = 256;

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
        match self.initialize_phase {
            1 => {
                if setup_data.request() == SetupData::REQUEST_GET_DESCRIPTOR {
                    if let DescriptorType::Device(ptr) =
                        Descriptor::new(buf as *const u8).specialize()
                    {
                        self.initialize_phase1();
                        return Ok(());
                    }
                }
                Err(Error::InvalidPhase)
            }
            2 => {
                if setup_data.request() == SetupData::REQUEST_GET_DESCRIPTOR {
                    let desc = Descriptor::new(buf as *const u8);
                    if let DescriptorType::Configuration(config_desc) = desc.specialize() {
                        return self.initialize_phase2(desc, config_desc, len);
                    }
                }
                Err(Error::InvalidPhase)
            }
            3 => {
                if setup_data.request() == SetupData::REQUEST_SET_CONFIGURATION {
                    return self.initialize_phase3();
                }
                Err(Error::InvalidPhase)
            }
            _ => Err(Error::NotImplemented),
        }
    }

    fn initialize_phase1(&mut self) -> Result<()> {
        self.initialize_phase = 2;
        self.get_descriptor(
            EndpointId::DEFAULT_CONTROL_PIPE_ID,
            ConfigurationDescriptor::TYPE,
            0,
            Some(self.data_buf),
            Self::DATA_BUF_LEN,
        )
    }

    fn initialize_phase2(
        &mut self,
        desc: Descriptor,
        config_desc: ConfigurationDescriptor,
        len: u32,
    ) -> Result<()> {
        let mut iter = desc.iter(len as usize);

        let mut class_driver_found = false;
        while let Some(desc_type) = iter.next() {
            if let DescriptorType::Interface(interface_desc) = desc_type {
                if let Some(class_driver) = ClassDriver::new(&interface_desc) {
                    class_driver_found = true;
                    for _ in 0..interface_desc.num_endpoints() {
                        match iter.next() {
                            Some(DescriptorType::Endpoint(ep_desc)) => {
                                let conf = EndpointConfig::from(&ep_desc);
                                self.class_drivers[conf.endpoint_id.number() as usize] =
                                    Some(class_driver);
                                self.ep_configs.add(conf);
                            }
                            Some(DescriptorType::Hid(_)) => {
                                // noop
                            }
                            _ => {}
                        }
                    }
                }
                break;
            }
        }
        if !class_driver_found {
            return Ok(());
        }
        self.initialize_phase = 3;
        self.set_configuration(
            EndpointId::DEFAULT_CONTROL_PIPE_ID,
            config_desc.configuration_value(),
        )
    }

    fn initialize_phase3(&mut self) -> Result<()> {
        for &config in self.ep_configs.as_slice() {
            if let Some(mut driver) = self.class_drivers[config.endpoint_id.number() as usize] {
                driver.set_endpoint(&config);
            }
        }
        self.initialize_phase = 4;
        self.is_initialized = true;
        Ok(())
    }

    fn get_descriptor(
        &mut self,
        endpoint_id: EndpointId,
        desc_type: u8,
        desc_index: u8,
        buf: Option<*const ()>,
        len: u32,
    ) -> Result<()> {
        let setup_data = SetupData::new()
            .set_request_type(
                RequestType::new()
                    .set_direction(RequestType::DIRECTION_IN)
                    .set_type(RequestType::TYPE_STANDARD)
                    .set_recipient(RequestType::RECIPIENT_DEVICE),
            )
            .set_request(SetupData::REQUEST_GET_DESCRIPTOR)
            .set_value(((desc_type as u16) << 8) | (desc_index as u16))
            .set_index(0)
            .set_length(len as u16);
        self.control_in(endpoint_id, setup_data, buf, len, None)
    }

    fn set_configuration(&mut self, endpoint_id: EndpointId, config_value: u8) -> Result<()> {
        let setup_data = SetupData::new()
            .set_request_type(
                RequestType::new()
                    .set_direction(RequestType::DIRECTION_OUT)
                    .set_type(RequestType::TYPE_STANDARD)
                    .set_recipient(RequestType::RECIPIENT_DEVICE),
            )
            .set_request(SetupData::REQUEST_SET_CONFIGURATION)
            .set_value(config_value as u16)
            .set_index(0)
            .set_length(0);
        self.control_out(endpoint_id, setup_data, None, 0, None)
    }

    fn control_in(
        &mut self,
        endpoint_id: EndpointId,
        setup_data: SetupData,
        buf: Option<*const ()>,
        len: u32,
        issuer: Option<ClassDriver>,
    ) -> Result<()> {
        if let Some(driver) = issuer {
            self.event_waiters.insert(setup_data, driver);
        }

        if 15 < endpoint_id.number() {
            return Err(Error::InvalidEndpointNumber);
        }

        let dci = endpoint_id.address() as usize;
        let tr = if let Some(ring) = &mut self.transfer_rings[dci - 1] {
            ring
        } else {
            return Err(Error::TransferRingNotSet);
        };

        match buf {
            Some(buf) => {
                let setup_stage_ptr = tr.push(
                    SetupStageTrb::from(&setup_data, SetupStageTrb::TRANSFER_TYPE_IN_DATA_STAGE)
                        .data(),
                );
                let data = DataStageTrb::from(buf, len, true).set_interrupt_on_completion(true);

                let data_stage_ptr = tr.push(data.data());
                tr.push(StatusStageTrb::new().data());

                self.setup_stage_map
                    .insert(data_stage_ptr, setup_stage_ptr as *const SetupStageTrb);
            }
            None => {
                let setup_stage_ptr = tr.push(
                    SetupStageTrb::from(&setup_data, SetupStageTrb::TRANSFER_TYPE_NO_DATA_STAGE)
                        .data(),
                );
                let status_trb_ptr = tr.push(
                    StatusStageTrb::new()
                        .set_direction(true)
                        .set_interrupt_on_completion(true)
                        .data(),
                );
                self.setup_stage_map
                    .insert(status_trb_ptr, setup_stage_ptr as *const SetupStageTrb);
            }
        }

        self.ring_doorbell(dci);
        Ok(())
    }

    fn control_out(
        &mut self,
        endpoint_id: EndpointId,
        setup_data: SetupData,
        buf: Option<*const ()>,
        len: u32,
        issuer: Option<ClassDriver>,
    ) -> Result<()> {
        if let Some(driver) = issuer {
            self.event_waiters.insert(setup_data, driver);
        }

        if 15 < endpoint_id.number() {
            return Err(Error::InvalidEndpointNumber);
        }

        let dci = endpoint_id.address() as usize;
        let tr = if let Some(ring) = &mut self.transfer_rings[dci - 1] {
            ring
        } else {
            return Err(Error::TransferRingNotSet);
        };

        match buf {
            Some(buf) => {
                let setup_stage_ptr = tr.push(
                    SetupStageTrb::from(&setup_data, SetupStageTrb::TRANSFER_TYPE_OUT_DATA_STAGE)
                        .data(),
                );
                let data = DataStageTrb::from(buf, len, true).set_interrupt_on_completion(true);

                let data_stage_ptr = tr.push(data.data());
                tr.push(StatusStageTrb::new().data());

                self.setup_stage_map
                    .insert(data_stage_ptr, setup_stage_ptr as *const SetupStageTrb);
            }
            None => {
                let setup_stage_ptr = tr.push(
                    SetupStageTrb::from(&setup_data, SetupStageTrb::TRANSFER_TYPE_NO_DATA_STAGE)
                        .data(),
                );
                let status_trb_ptr = tr.push(
                    StatusStageTrb::new()
                        .set_interrupt_on_completion(true)
                        .data(),
                );
                self.setup_stage_map
                    .insert(status_trb_ptr, setup_stage_ptr as *const SetupStageTrb);
            }
        }

        self.ring_doorbell(dci);
        Ok(())
    }

    fn interrupt_in(&mut self, endpoint_id: EndpointId, buf: *const (), len: u32) -> Result<()> {
        let dci = endpoint_id.address() as usize;
        let tr = if let Some(ring) = &mut self.transfer_rings[dci - 1] {
            ring
        } else {
            return Err(Error::TransferRingNotSet);
        };

        let normal_trb = NormalTrb::new()
            .set_pointer(&buf)
            .set_transfer_length(len)
            .set_interrupt_on_short_packet(true)
            .set_interrupt_on_completion(true);

        tr.push(normal_trb.data());
        self.ring_doorbell(dci);
        Ok(())
    }

    fn ring_doorbell(&mut self, dci: usize) {
        self.dbreg.update(|reg| {
            reg.set_doorbell_target(dci as u8);
            reg.set_doorbell_stream_id(0);
        });
    }
}
