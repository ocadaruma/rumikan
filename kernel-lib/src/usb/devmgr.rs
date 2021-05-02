use crate::usb::classdriver::ClassDriver;
use crate::usb::context::{DeviceContext, InputContext, SlotContext};
use crate::usb::descriptor::{
    ConfigurationDescriptor, Descriptor, DescriptorType, DeviceDescriptor,
};
use crate::usb::endpoint::{EndpointConfig, EndpointId, EndpointNumber, EndpointType};
use crate::usb::mem::{allocate, allocate_array};
use crate::usb::port::{Port, PortSpeed};
use crate::usb::ring::{
    DataStageTrb, NormalTrb, RequestType, Ring, SetupData, SetupStageTrb, StatusStageTrb,
    TransferEventTrb, Trb, TrbType,
};
use crate::usb::{IdentityMapper, SlotId};
use crate::util::{ArrayMap, ArrayVec};
use core::mem::size_of;
use core::ptr::{null, null_mut};

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
    UnknownXHCISpeedID,
    ArrayMapError(crate::util::ArrayMapError),
}

pub type Result<T> = core::result::Result<T, Error>;

const NUM_DEVICE_SLOTS: usize = 8;
const DEVICES_CAPACITY: usize = NUM_DEVICE_SLOTS + 1;

pub struct DeviceManager {
    max_slots: usize,
    device_contexts: *mut *mut DeviceContext,
    devices: ArrayMap<SlotId, UsbDevice, DEVICES_CAPACITY>,
}

type DoorbellRegister = xhci::accessor::Single<xhci::registers::doorbell::Register, IdentityMapper>;

impl DeviceManager {
    pub fn new() -> DeviceManager {
        DeviceManager {
            max_slots: NUM_DEVICE_SLOTS,
            device_contexts: null_mut(),
            devices: ArrayMap::new(),
        }
    }

    pub fn max_slots(&self) -> usize {
        self.max_slots
    }

    pub fn dcbaa_ptr(&self) -> u64 {
        self.device_contexts as u64
    }

    pub fn initialize(&mut self) -> Result<()> {
        let ctx_ptr = match allocate::<*mut DeviceContext>(
            size_of::<*mut DeviceContext>() * (self.max_slots + 1),
            Some(64),
            Some(4096),
        ) {
            Ok(ptr) => ptr,
            Err(err) => return Err(Error::AllocError(err)),
        };
        self.device_contexts = ctx_ptr;

        Ok(())
    }

    pub fn allocate_device(&mut self, slot_id: SlotId, dbreg: DoorbellRegister) -> Result<()> {
        let device_context =
            allocate_array::<DeviceContext>(1, Some(64), Some(4096)).map_err(Error::AllocError)?;
        let input_context =
            allocate_array::<InputContext>(1, Some(64), Some(4096)).map_err(Error::AllocError)?;
        let data_buf = allocate::<()>(256, None, None).map_err(Error::AllocError)?;

        let dev = UsbDevice {
            class_drivers: ArrayMap::new(),
            transfer_rings: ArrayMap::new(),
            dbreg,
            data_buf,
            ep_configs: ArrayVec::new(),
            setup_stage_map: ArrayMap::new(),
            event_waiters: ArrayMap::new(),
            device_context,
            input_context,
            is_initialized: false,
            initialize_phase: 0,
        };

        unsafe {
            self.device_contexts
                .add(slot_id.value() as usize)
                .write(device_context);
        }

        self.devices
            .insert(slot_id, dev)
            .map_err(Error::ArrayMapError)
            .map(|_| ())
    }

    pub fn find_by_slot(&mut self, slot_id: SlotId) -> Option<&mut UsbDevice> {
        self.devices.get_mut(&slot_id)
    }
}

#[derive(Debug)]
pub struct UsbDevice {
    class_drivers: ArrayMap<EndpointNumber, ClassDriver, { EndpointNumber::MAX as usize }>,
    transfer_rings: ArrayMap<EndpointId, Ring, { EndpointId::MAX as usize }>,
    dbreg: DoorbellRegister,
    data_buf: *const (),
    ep_configs: ArrayVec<EndpointConfig, { EndpointNumber::MAX as usize }>,
    setup_stage_map: ArrayMap<*const Trb, *const SetupStageTrb, 16>,
    event_waiters: ArrayMap<SetupData, ClassDriver, 4>,
    device_context: *mut DeviceContext,
    input_context: *mut InputContext,
    is_initialized: bool,
    initialize_phase: u8,
}

impl UsbDevice {
    const DATA_BUF_LEN: u32 = 256;

    pub fn device_context(&self) -> &DeviceContext {
        unsafe { self.device_context.as_ref().unwrap() }
    }

    pub fn input_context_ptr(&self) -> u64 {
        self.input_context as u64
    }

    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    pub fn start_initialize(&mut self) -> Result<()> {
        self.is_initialized = false;
        self.initialize_phase = 1;
        self.get_descriptor(
            EndpointId::DEFAULT_CONTROL_PIPE_ID,
            DeviceDescriptor::TYPE,
            0,
            Some(self.data_buf),
            Self::DATA_BUF_LEN,
        )
    }

    pub fn address_device(&mut self, port: Port) {
        let ep0 = EndpointId::from(EndpointNumber::new(0), false);
        let tr_ptr = self.alloc_transfer_ring(ep0, 32).ptr_as_u64();
        let slot_ctx = unsafe { self.input_context.as_mut().unwrap() }.enable_slot_context();
        let ep0_ctx = unsafe { self.input_context.as_mut().unwrap() }.enable_endpoint(ep0);

        slot_ctx.set_route_string(0);
        slot_ctx.set_root_hub_port_num(port.port_num());
        slot_ctx.set_context_entries(1);
        slot_ctx.set_speed(port.port_speed().unwrap());

        ep0_ctx.set_endpoint_type(4); // Control Endpoint. Bidi
        ep0_ctx.set_max_packet_size(match slot_ctx.speed() {
            Ok(PortSpeed::SuperSpeed) => 512,
            Ok(PortSpeed::HighSpeed) => 64,
            _ => 8,
        });
        ep0_ctx.set_max_burst_size(0);
        ep0_ctx.set_transfer_ring_buffer(tr_ptr);
        ep0_ctx.set_dequeue_cycle_state(true);
        ep0_ctx.set_interval(0);
        ep0_ctx.set_max_primary_streams(0);
        ep0_ctx.set_mult(0);
        ep0_ctx.set_error_count(3);
    }

    pub fn configure_endpoints(&mut self, port: Port) -> Result<()> {
        let slot_ctx = unsafe {
            self.input_context.as_mut().unwrap().slot_context =
                self.device_context.as_ref().unwrap().slot_context;
            self.input_context.as_mut().unwrap().enable_slot_context()
        };

        slot_ctx.set_context_entries(EndpointId::MAX);
        let port_speed = port.port_speed().ok_or(Error::UnknownXHCISpeedID)?;

        for i in 0..self.ep_configs.len() {
            let ep_config = self.ep_configs[i];

            let ep_ctx = unsafe {
                self.input_context
                    .as_mut()
                    .unwrap()
                    .enable_endpoint(ep_config.endpoint_id)
            };
            match ep_config.endpoint_type {
                EndpointType::Control => ep_ctx.set_endpoint_type(4),
                EndpointType::Isochronous => {
                    ep_ctx.set_endpoint_type(if ep_config.endpoint_id.is_in() { 5 } else { 1 })
                }
                EndpointType::Bulk => {
                    ep_ctx.set_endpoint_type(if ep_config.endpoint_id.is_in() { 6 } else { 2 })
                }
                EndpointType::Interrupt => {
                    ep_ctx.set_endpoint_type(if ep_config.endpoint_id.is_in() { 7 } else { 3 })
                }
            }

            ep_ctx.set_max_packet_size(ep_config.max_packet_size as u16);
            ep_ctx.set_interval(
                port_speed.convert_interval(ep_config.endpoint_type, ep_config.interval) as u8,
            );
            ep_ctx.set_average_trb_length(1);
            let tr = self.alloc_transfer_ring(ep_config.endpoint_id, 32);

            ep_ctx.set_transfer_ring_buffer(tr.ptr_as_u64());
            ep_ctx.set_dequeue_cycle_state(true);
            ep_ctx.set_max_primary_streams(0);
            ep_ctx.set_mult(0);
            ep_ctx.set_error_count(3);
        }
        Ok(())
    }

    pub fn on_transfer_event_received(&mut self, trb: &TransferEventTrb) -> Result<()> {
        let residual_length = trb.transfer_length();

        if !(trb.completion_code() == 1 || trb.completion_code() == 13) {
            return Err(Error::TransferFailed);
        }

        if let TrbType::Normal(normal_trb) = unsafe { *trb.trb_pointer() }.specialize() {
            let transfer_length = normal_trb.transfer_length() - residual_length;
            return self.on_interrupt_completed(trb.endpoint_id(), transfer_length);
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
                    setup_data,
                    data_stage_trb.data_buffer_pointer(),
                    transfer_length,
                )
            }
            TrbType::StatusStage(_) => self.on_control_completed(setup_data, null(), 0),
            _ => Err(Error::NotImplemented),
        }
    }

    pub fn on_endpoints_configured(&mut self) -> Result<()> {
        for i in 0..self.ep_configs.len() {
            let conf = self.ep_configs[i];
            let driver = self
                .class_drivers
                .get_mut(&conf.endpoint_id.number())
                .unwrap();
            let setup_data = SetupData::new()
                .set_request_type(
                    RequestType::new()
                        .set_direction(RequestType::DIRECTION_OUT)
                        .set_type(RequestType::TYPE_CLASS)
                        .set_recipient(RequestType::RECIPIENT_INTERFACE),
                )
                .set_request(SetupData::REQUEST_SET_PROTOCOL)
                .set_value(0)
                .set_index(driver.interface_index() as u16)
                .set_length(0);
            let driver = *driver;
            self.control_out(
                EndpointId::DEFAULT_CONTROL_PIPE_ID,
                setup_data,
                None,
                0,
                Some(driver),
            )?;
        }
        Ok(())
    }

    fn on_interrupt_completed(&mut self, endpoint_id: EndpointId, len: u32) -> Result<()> {
        if let Some(driver) = self.class_drivers.get(&endpoint_id.number()) {
            if endpoint_id.is_in() {
                driver
                    .on_interrupt_completed(endpoint_id, len)
                    .map_err(Error::ClassDriverError)?;
                self.interrupt_in(
                    driver.endpoint_interrupt_in(),
                    driver.buffer(),
                    driver.in_packet_size() as u32,
                )
            } else {
                Ok(())
            }
        } else {
            Err(Error::NoWaiter)
        }
    }

    fn on_control_completed(
        &mut self,
        setup_data: SetupData,
        buf: *const (),
        len: u32,
    ) -> Result<()> {
        if self.is_initialized {
            if let Some(waiter) = self.event_waiters.get_mut(&setup_data) {
                let mut waiter = *waiter;
                return self.interrupt_in(
                    waiter.endpoint_interrupt_in(),
                    waiter.buffer(),
                    waiter.in_packet_size() as u32,
                );
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
                                self.class_drivers
                                    .insert(conf.endpoint_id.number(), class_driver);
                                self.ep_configs.push(conf);
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

        if EndpointNumber::MAX_ENDPOINT < endpoint_id.number() {
            return Err(Error::InvalidEndpointNumber);
        }

        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
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

        self.ring_doorbell(endpoint_id);
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

        if EndpointNumber::MAX_ENDPOINT < endpoint_id.number() {
            return Err(Error::InvalidEndpointNumber);
        }

        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
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

        self.ring_doorbell(endpoint_id);
        Ok(())
    }

    fn interrupt_in(&mut self, endpoint_id: EndpointId, buf: *const (), len: u32) -> Result<()> {
        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
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
        self.ring_doorbell(endpoint_id);
        Ok(())
    }

    fn ring_doorbell(&mut self, endpoint_id: EndpointId) {
        self.dbreg.update(|reg| {
            reg.set_doorbell_target(endpoint_id.address());
            reg.set_doorbell_stream_id(0);
        });
    }

    fn alloc_transfer_ring(&mut self, endpoint_id: EndpointId, buf_size: usize) -> &mut Ring {
        let mut ring = Ring::new();
        ring.initialize(buf_size);

        self.transfer_rings.insert(endpoint_id, ring);
        self.transfer_rings
            .get_mut(&endpoint_id)
            .expect("Existence is guaranteed here")
    }
}
