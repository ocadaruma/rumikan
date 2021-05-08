use crate::error::ErrorContext;
use crate::usb::classdriver::ClassDriver;
use crate::usb::context::{DeviceContext, InputContext, InputControlContext};
use crate::usb::descriptor::{
    ConfigurationDescriptor, Descriptor, DescriptorType, DeviceDescriptor,
};
use crate::usb::endpoint::{EndpointConfig, EndpointId, EndpointNumber, EndpointType};
use crate::usb::mem::{allocate, allocate_array};
use crate::usb::port::{Port, PortSpeed};
use crate::usb::trb::ring::Ring;
use crate::usb::trb::{
    DataStageTrb, NormalTrb, RequestType, SetupData, SetupStageTrb, StatusStageTrb,
    TransferEventTrb,
};
use crate::usb::xhci::{Accessor, DoorbellRegister};
use crate::usb::SlotId;
use crate::util::collection::{ArrayMap, ArrayVec};
use core::mem::size_of;
use core::ptr::{null, null_mut};

#[derive(Debug)]
pub enum ErrorType {
    AllocError(crate::usb::mem::Error),
    TransferFailed(u8),
    NoWaiter,
    NoCorrespondingSetupStage,
    NotImplemented,
    ClassDriverError(crate::usb::classdriver::Error),
    InvalidPhase,
    InvalidEndpointNumber,
    TransferRingNotSet,
    UnknownXHCISpeedID,
    ArrayMapError(crate::util::collection::ArrayMapError),
    ArrayVecError(crate::util::collection::ArrayVecError),
    TrbError(crate::usb::trb::Error),
}

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

const NUM_DEVICE_SLOTS: usize = 8;
const DEVICES_CAPACITY: usize = NUM_DEVICE_SLOTS + 1;

pub struct DeviceManager {
    max_slots: usize,
    device_contexts: *mut *mut DeviceContext,
    devices: ArrayMap<SlotId, UsbDevice, DEVICES_CAPACITY>,
}

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
        let ctx_ptr = allocate::<*mut DeviceContext>(
            size_of::<*mut DeviceContext>() * (self.max_slots + 1),
            Some(64),
            Some(4096),
        )
        .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;
        self.device_contexts = ctx_ptr;

        Ok(())
    }

    pub fn allocate_device(
        &mut self,
        slot_id: SlotId,
        dbreg: Accessor<DoorbellRegister>,
    ) -> Result<()> {
        let device_context = allocate_array::<DeviceContext>(1, Some(64), Some(4096))
            .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;
        let input_context = allocate_array::<InputContext>(1, Some(64), Some(4096))
            .map_err(|e| mkerror!(ErrorType::AllocError(e)))?;
        let data_buf =
            allocate::<()>(256, None, None).map_err(|e| mkerror!(ErrorType::AllocError(e)))?;

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
            .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))
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
    dbreg: Accessor<DoorbellRegister>,
    data_buf: *const (),
    ep_configs: ArrayVec<EndpointConfig, { EndpointNumber::MAX as usize }>,
    setup_stage_map: ArrayMap<u64, SetupStageTrb, 16>,
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

    pub fn address_device(&mut self, port: Port) -> Result<()> {
        let ep0 = EndpointId::DEFAULT_CONTROL_PIPE_ID;
        let tr_ptr = self.alloc_transfer_ring(ep0, 32)?.buffer_pointer();
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

        Ok(())
    }

    pub fn configure_endpoints(&mut self, port: Port) -> Result<()> {
        let input_ctx = unsafe { &mut *self.input_context };
        let device_ctx = unsafe { &mut *self.device_context };

        input_ctx.input_control_context = InputControlContext::default();
        input_ctx.slot_context = device_ctx.slot_context;
        let slot_ctx = input_ctx.enable_slot_context();

        slot_ctx.set_context_entries(EndpointId::MAX);
        let port_speed = port
            .port_speed()
            .ok_or_else(|| mkerror!(ErrorType::UnknownXHCISpeedID))?;

        for i in 0..self.ep_configs.len() {
            let ep_config = self.ep_configs[i];

            input_ctx.enable_endpoint(ep_config.endpoint_id);
            let ep_ctx = input_ctx.enable_endpoint(ep_config.endpoint_id);
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
            let tr = self.alloc_transfer_ring(ep_config.endpoint_id, 32)?;

            ep_ctx.set_transfer_ring_buffer(tr.buffer_pointer());
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
            return Err(mkerror!(ErrorType::TransferFailed(trb.completion_code())));
        }

        if let Some(normal_trb) = trb.issuer_trb().specialize::<NormalTrb>() {
            let transfer_length = normal_trb.transfer_length() - residual_length;
            return self.on_interrupt_completed(trb.endpoint_id(), transfer_length);
        }

        let setup_stage_trb = self
            .setup_stage_map
            .remove(&trb.issuer_pointer())
            .ok_or_else(|| mkerror!(ErrorType::NoCorrespondingSetupStage))?;

        let setup_data = setup_stage_trb.setup_data();
        if let Some(data_stage_trb) = trb.issuer_trb().specialize::<DataStageTrb>() {
            let transfer_length = data_stage_trb.trb_transfer_length() - residual_length;
            self.on_control_completed(
                setup_data,
                data_stage_trb.data_buffer_pointer() as *const (),
                transfer_length,
            )
        } else if trb.issuer_trb().specialize::<StatusStageTrb>().is_some() {
            self.on_control_completed(setup_data, null(), 0)
        } else {
            Err(mkerror!(ErrorType::NotImplemented))
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
                .with_request_type(
                    RequestType::new()
                        .with_direction(RequestType::DIRECTION_HOST_TO_DEVICE)
                        .with_type(RequestType::TYPE_CLASS)
                        .with_recipient(RequestType::RECIPIENT_INTERFACE),
                )
                .with_request(SetupData::REQUEST_SET_PROTOCOL)
                .with_value(0)
                .with_index(driver.interface_index() as u16)
                .with_length(0);
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
                let driver = *driver;
                driver
                    .on_interrupt_completed(endpoint_id, len)
                    .map_err(|e| mkerror!(ErrorType::ClassDriverError(e)))?;
                self.interrupt_in(
                    driver.endpoint_interrupt_in(),
                    driver.buffer(),
                    driver.in_packet_size() as u32,
                )
            } else {
                Ok(())
            }
        } else {
            Err(mkerror!(ErrorType::NoWaiter))
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
                let waiter = *waiter;
                return self.interrupt_in(
                    waiter.endpoint_interrupt_in(),
                    waiter.buffer(),
                    waiter.in_packet_size() as u32,
                );
            }
            return Err(mkerror!(ErrorType::NoWaiter));
        }
        match self.initialize_phase {
            1 => {
                if setup_data.request() == SetupData::REQUEST_GET_DESCRIPTOR {
                    if let DescriptorType::Device(_) =
                        Descriptor::new(buf as *const u8).specialize()
                    {
                        return self.initialize_phase1();
                    }
                }
                Err(mkerror!(ErrorType::InvalidPhase))
            }
            2 => {
                if setup_data.request() == SetupData::REQUEST_GET_DESCRIPTOR {
                    let desc = Descriptor::new(buf as *const u8);
                    if let DescriptorType::Configuration(config_desc) = desc.specialize() {
                        return self.initialize_phase2(desc, config_desc, len);
                    }
                }
                Err(mkerror!(ErrorType::InvalidPhase))
            }
            3 => {
                if setup_data.request() == SetupData::REQUEST_SET_CONFIGURATION {
                    return self.initialize_phase3();
                }
                Err(mkerror!(ErrorType::InvalidPhase))
            }
            _ => Err(mkerror!(ErrorType::NotImplemented)),
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
        while let Some(DescriptorType::Interface(interface_desc)) = iter.next() {
            if let Some(class_driver) = ClassDriver::new(&interface_desc) {
                class_driver_found = true;
                let mut num_endpoints = 0;
                while num_endpoints < interface_desc.num_endpoints() {
                    match iter.next() {
                        Some(DescriptorType::Endpoint(ep_desc)) => {
                            num_endpoints += 1;
                            let conf = EndpointConfig::from(&ep_desc);
                            self.class_drivers
                                .insert(conf.endpoint_id.number(), class_driver)
                                .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))?;
                            self.ep_configs
                                .push(conf)
                                .map_err(|e| mkerror!(ErrorType::ArrayVecError(e)))?;
                        }
                        Some(DescriptorType::Hid(_)) => {
                            // noop
                        }
                        _ => {}
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
        for i in 0..self.ep_configs.len() {
            let config = self.ep_configs[i];
            let driver = self
                .class_drivers
                .get_mut(&config.endpoint_id.number())
                .unwrap();
            driver.set_endpoint(&config);
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
            .with_request_type(
                RequestType::new()
                    .with_direction(RequestType::DIRECTION_DEVICE_TO_HOST)
                    .with_type(RequestType::TYPE_STANDARD)
                    .with_recipient(RequestType::RECIPIENT_DEVICE),
            )
            .with_request(SetupData::REQUEST_GET_DESCRIPTOR)
            .with_value(((desc_type as u16) << 8) | (desc_index as u16))
            .with_index(0)
            .with_length(len as u16);
        self.control_in(endpoint_id, setup_data, buf, len, None)
    }

    fn set_configuration(&mut self, endpoint_id: EndpointId, config_value: u8) -> Result<()> {
        let setup_data = SetupData::new()
            .with_request_type(
                RequestType::new()
                    .with_direction(RequestType::DIRECTION_HOST_TO_DEVICE)
                    .with_type(RequestType::TYPE_STANDARD)
                    .with_recipient(RequestType::RECIPIENT_DEVICE),
            )
            .with_request(SetupData::REQUEST_SET_CONFIGURATION)
            .with_value(config_value as u16)
            .with_index(0)
            .with_length(0);
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
            self.event_waiters
                .insert(setup_data, driver)
                .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))?;
        }

        if EndpointNumber::MAX_ENDPOINT < endpoint_id.number() {
            return Err(mkerror!(ErrorType::InvalidEndpointNumber));
        }

        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
            ring
        } else {
            return Err(mkerror!(ErrorType::TransferRingNotSet));
        };

        match buf {
            Some(buf) => {
                let setup_stage = tr
                    .push(setup_data.trb(SetupStageTrb::TRANSFER_TYPE_IN_DATA_STAGE))
                    .trb;
                let data = DataStageTrb::new()
                    .with_direction_in(true)
                    .with_data_buffer_pointer(buf as u64)
                    .with_trb_transfer_length(len)
                    .with_interrupt_on_completion(true);

                let data_stage_ptr = tr.push(data).ptr;
                tr.push(StatusStageTrb::new());

                self.setup_stage_map
                    .insert(data_stage_ptr, setup_stage)
                    .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))?;
            }
            None => {
                let setup_stage = tr
                    .push(setup_data.trb(SetupStageTrb::TRANSFER_TYPE_NO_DATA_STAGE))
                    .trb;
                let status_trb_ptr = tr
                    .push(
                        StatusStageTrb::new()
                            .with_direction_in(true)
                            .with_interrupt_on_completion(true),
                    )
                    .ptr;
                self.setup_stage_map
                    .insert(status_trb_ptr, setup_stage)
                    .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))?;
            }
        }

        self.dbreg.as_mut().ring(endpoint_id.address(), 0);
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
            self.event_waiters
                .insert(setup_data, driver)
                .map_err(ErrorType::ArrayMapError)
                .map_err(|e| mkerror!(e))?;
        }

        if EndpointNumber::MAX_ENDPOINT < endpoint_id.number() {
            return Err(mkerror!(ErrorType::InvalidEndpointNumber));
        }

        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
            ring
        } else {
            return Err(mkerror!(ErrorType::TransferRingNotSet));
        };

        match buf {
            Some(buf) => {
                let setup_stage = tr
                    .push(setup_data.trb(SetupStageTrb::TRANSFER_TYPE_OUT_DATA_STAGE))
                    .trb;
                let data = DataStageTrb::new()
                    .with_data_buffer_pointer(buf as u64)
                    .with_trb_transfer_length(len)
                    .with_direction_in(true)
                    .with_interrupt_on_completion(true);

                let data_stage_ptr = tr.push(data).ptr;
                tr.push(StatusStageTrb::new());

                self.setup_stage_map
                    .insert(data_stage_ptr, setup_stage)
                    .map_err(ErrorType::ArrayMapError)
                    .map_err(|e| mkerror!(e))?;
            }
            None => {
                let setup_stage = tr
                    .push(setup_data.trb(SetupStageTrb::TRANSFER_TYPE_NO_DATA_STAGE))
                    .trb;
                let status_trb_ptr = tr
                    .push(StatusStageTrb::new().with_interrupt_on_completion(true))
                    .ptr;
                self.setup_stage_map
                    .insert(status_trb_ptr, setup_stage)
                    .map_err(ErrorType::ArrayMapError)
                    .map_err(|e| mkerror!(e))?;
            }
        }
        self.dbreg.as_mut().ring(endpoint_id.address(), 0);
        Ok(())
    }

    fn interrupt_in(&mut self, endpoint_id: EndpointId, buf: *const (), len: u32) -> Result<()> {
        let tr = if let Some(ring) = self.transfer_rings.get_mut(&endpoint_id) {
            ring
        } else {
            return Err(mkerror!(ErrorType::TransferRingNotSet));
        };

        let normal_trb = NormalTrb::new()
            .with_pointer(buf as u64)
            .with_transfer_length(len)
            .with_interrupt_on_short_packet(true)
            .with_interrupt_on_completion(true);

        tr.push(normal_trb);
        self.dbreg.as_mut().ring(endpoint_id.address(), 0);
        Ok(())
    }

    fn alloc_transfer_ring(
        &mut self,
        endpoint_id: EndpointId,
        buf_size: usize,
    ) -> Result<&mut Ring> {
        let mut ring = Ring::new();
        ring.initialize(buf_size)
            .map_err(|e| mkerror!(ErrorType::TrbError(e)))?;

        self.transfer_rings
            .insert(endpoint_id, ring)
            .map_err(|e| mkerror!(ErrorType::ArrayMapError(e)))?;
        Ok(self
            .transfer_rings
            .get_mut(&endpoint_id)
            .expect("Existence is guaranteed here"))
    }
}

#[cfg(test)]
mod tests {
    use crate::usb::context::InputContext;
    use crate::usb::mem::{allocate_array, free_all};

    #[test]
    fn enable_slot_context() {
        free_all();

        let input_context_ptr = allocate_array::<InputContext>(1, Some(64), Some(4096)).unwrap();
        let input_context = unsafe { input_context_ptr.as_mut().unwrap() };
        input_context.slot_context.set_root_hub_port_num(42);

        assert_eq!(
            unsafe { input_context_ptr.read() }
                .slot_context
                .root_hub_port_num(),
            42
        );
    }
}
