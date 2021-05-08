#[macro_use]
mod bitfield;
pub mod classdriver;
mod context;
mod descriptor;
mod devmgr;
mod endpoint;
mod mem;
mod port;
mod trb;
mod xhci;

use crate::error::ErrorContext;
use crate::usb::devmgr::DeviceManager;
use crate::usb::port::Port;
use crate::usb::trb::ring::{EventRing, Ring};
use crate::usb::trb::{
    AddressDeviceCommandTrb, CommandCompletionEventTrb, ConfigureEndpointCommandTrb,
    EnableSlotCommandTrb, PortStatusChangeEventTrb, TransferEventTrb,
};
use crate::usb::xhci::{ExtendedCapability, Registers};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SlotId(u8);
impl SlotId {
    pub fn new(value: u8) -> Self {
        Self(value)
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug)]
pub enum ErrorType {
    InvalidPhase,
    NotImplemented,
    InvalidSlotId,
    DeviceError(devmgr::Error),
}

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

pub struct Xhc {
    registers: Registers,
    device_manager: DeviceManager,
    command_ring: Ring,
    event_ring: EventRing,
    port_config_phase: [ConfigPhase; 256],
    addressing_port: Option<u8>,
}

impl Xhc {
    pub fn new(mmio_base: usize) -> Xhc {
        Xhc {
            registers: Registers::new(mmio_base),
            device_manager: DeviceManager::new(),
            command_ring: Ring::new(),
            event_ring: EventRing::new(),
            port_config_phase: [ConfigPhase::NotConnected; 256],
            addressing_port: None,
        }
    }

    pub fn initialize(&mut self) {
        self.device_manager
            .initialize()
            .expect("Failed to initialize device manager");
        self.request_hc_ownership();
        self.initialize_host_controller();
        self.set_enabled_device_slots();
        // TODO: initialize scratchpad buffer
        self.set_dcbaap();
        self.init_command_ring();
        self.init_event_ring();
        self.init_interrupter();
    }

    pub fn run(&mut self) {
        self.registers.operational.as_mut().usbcmd.update(|u| {
            u.set_run_stop(true);
        });

        while self
            .registers
            .operational
            .as_ref()
            .usbsts
            .read()
            .host_controller_halted()
        {}
        debug!("xHC started");
    }

    pub fn max_ports(&self) -> u8 {
        self.registers
            .capability
            .as_ref()
            .hcsparams1
            .read()
            .max_ports()
    }

    pub fn port_at(&self, num: u8) -> Port {
        Port::new(
            num,
            self.registers
                .port_register_set
                .at(num as usize - 1)
                .unwrap(),
        )
    }

    pub fn configure_port(&mut self, port: &mut Port) -> Result<()> {
        if self.port_config_phase[port.port_num() as usize] == ConfigPhase::NotConnected {
            self.reset_port(port)
        } else {
            Ok(())
        }
    }

    pub fn poll(&mut self) -> Result<()> {
        if let Some(trb) = self.event_ring.poll() {
            if let Some(trb) = trb.specialize::<TransferEventTrb>() {
                self.on_transfer_event(trb)
            } else if let Some(trb) = trb.specialize::<CommandCompletionEventTrb>() {
                self.on_command_completion_event(trb)
            } else if let Some(trb) = trb.specialize::<PortStatusChangeEventTrb>() {
                self.on_port_status_change_event(trb)
            } else {
                debug!("Unexpected trb type: {:?}", trb.trb_type());
                Err(mkerror!(ErrorType::NotImplemented))
            }
        } else {
            Ok(())
        }
    }

    fn on_transfer_event(&mut self, trb: &TransferEventTrb) -> Result<()> {
        let slot_id = trb.slot_id();
        debug!(
            "TransferEvent: slot_id = {}, issuer = {:?}",
            trb.slot_id().value(),
            trb.issuer_trb().trb_type()
        );

        let dev = self
            .device_manager
            .find_by_slot(slot_id)
            .ok_or_else(|| mkerror!(ErrorType::InvalidSlotId))?;
        dev.on_transfer_event_received(trb)
            .map_err(ErrorType::DeviceError)
            .map_err(|e| mkerror!(e))?;

        let port_id = dev.device_context().slot_context.root_hub_port_num();
        if dev.is_initialized()
            && self.port_config_phase[port_id as usize] == ConfigPhase::InitializingDevice
        {
            self.configure_endpoints(slot_id, port_id)
        } else {
            Ok(())
        }
    }

    fn on_command_completion_event(&mut self, trb: &CommandCompletionEventTrb) -> Result<()> {
        debug!(
            "CommandCompletionEvent: slot_id = {}, issuer = {:?}",
            trb.slot_id().value(),
            trb.issuer().trb_type()
        );

        if trb.issuer().specialize::<EnableSlotCommandTrb>().is_some() {
            if let Some(addressing_port) = self.addressing_port {
                if self.port_config_phase[addressing_port as usize] == ConfigPhase::EnablingSlot {
                    return self.address_device(addressing_port, trb.slot_id());
                }
            }
        } else if trb
            .issuer()
            .specialize::<AddressDeviceCommandTrb>()
            .is_some()
        {
            let port_id = {
                let dev = self
                    .device_manager
                    .find_by_slot(trb.slot_id())
                    .ok_or_else(|| mkerror!(ErrorType::InvalidSlotId))?;
                dev.device_context().slot_context.root_hub_port_num()
            };

            if self.addressing_port == Some(port_id)
                && self.port_config_phase[port_id as usize] == ConfigPhase::AddressingDevice
            {
                self.addressing_port = None;
                for i in 0..self.port_config_phase.len() {
                    if self.port_config_phase[i] == ConfigPhase::WaitingAddressed {
                        let mut port = self.port_at(i as u8);
                        self.reset_port(&mut port)?;
                        break;
                    }
                }

                self.port_config_phase[port_id as usize] = ConfigPhase::InitializingDevice;
                return self
                    .device_manager
                    .find_by_slot(trb.slot_id())
                    .expect("Existence is guaranteed here")
                    .start_initialize()
                    .map_err(|e| mkerror!(ErrorType::DeviceError(e)));
            }
        } else if trb
            .issuer()
            .specialize::<ConfigureEndpointCommandTrb>()
            .is_some()
        {
            let dev = self
                .device_manager
                .find_by_slot(trb.slot_id())
                .ok_or_else(|| mkerror!(ErrorType::InvalidSlotId))?;
            let port_id = dev.device_context().slot_context.root_hub_port_num();
            if self.port_config_phase[port_id as usize] == ConfigPhase::ConfiguringEndpoints {
                self.port_config_phase[port_id as usize] = ConfigPhase::Configured;
                return dev
                    .on_endpoints_configured()
                    .map_err(ErrorType::DeviceError)
                    .map_err(|e| mkerror!(e));
            }
        }
        Err(mkerror!(ErrorType::InvalidPhase))
    }

    fn address_device(&mut self, port_id: u8, slot_id: SlotId) -> Result<()> {
        let port = self.port_at(port_id);
        let dbreg = self
            .registers
            .doorbell
            .at(slot_id.value() as usize)
            .ok_or_else(|| mkerror!(ErrorType::InvalidSlotId))?;
        self.device_manager
            .allocate_device(slot_id, dbreg)
            .map_err(|e| mkerror!(ErrorType::DeviceError(e)))?;

        let dev = self
            .device_manager
            .find_by_slot(slot_id)
            .expect("Existence is guaranteed here");
        dev.address_device(port)
            .map_err(|e| mkerror!(ErrorType::DeviceError(e)))?;

        self.port_config_phase[port_id as usize] = ConfigPhase::AddressingDevice;
        self.command_ring.push(AddressDeviceCommandTrb::new(
            slot_id,
            dev.input_context_ptr(),
        ));
        self.registers.doorbell.at(0).unwrap().as_mut().ring(0, 0);

        Ok(())
    }

    fn on_port_status_change_event(&mut self, trb: &PortStatusChangeEventTrb) -> Result<()> {
        let port_id = trb.port_id();
        debug!("PortStatusChangeEvent: port_id = {}", port_id);

        let mut port = self.port_at(port_id);

        match self.port_config_phase[port_id as usize] {
            ConfigPhase::NotConnected => self.reset_port(&mut port),
            ConfigPhase::ResettingPort => {
                self.enable_slot(&mut port);
                Ok(())
            }
            phase => {
                debug!("port = {}, phase: {:?}", port_id, phase);
                Err(mkerror!(ErrorType::InvalidPhase))
            }
        }
    }

    fn configure_endpoints(&mut self, slot_id: SlotId, port_id: u8) -> Result<()> {
        let port = self.port_at(port_id);
        let dev = self
            .device_manager
            .find_by_slot(slot_id)
            .expect("Device existence is guaranteed here");
        let input_context_ptr = dev.input_context_ptr();
        dev.configure_endpoints(port)
            .map_err(|e| mkerror!(ErrorType::DeviceError(e)))?;

        self.port_config_phase[port_id as usize] = ConfigPhase::ConfiguringEndpoints;
        self.command_ring
            .push(ConfigureEndpointCommandTrb::new(slot_id, input_context_ptr));
        self.registers.doorbell.at(0).unwrap().as_mut().ring(0, 0);

        Ok(())
    }

    fn enable_slot(&mut self, port: &mut Port) {
        if port.is_enabled() && port.is_port_reset_changed() {
            port.clear_port_reset_change();
            self.port_config_phase[port.port_num() as usize] = ConfigPhase::EnablingSlot;

            self.command_ring.push(EnableSlotCommandTrb::new());
            self.registers.doorbell.at(0).unwrap().as_mut().ring(0, 0);
        }
    }

    fn reset_port(&mut self, port: &mut Port) -> Result<()> {
        if port.is_connected() {
            match self.addressing_port {
                Some(_) => {
                    self.port_config_phase[port.port_num() as usize] =
                        ConfigPhase::WaitingAddressed;
                }
                None => match self.port_config_phase[port.port_num() as usize] {
                    ConfigPhase::NotConnected | ConfigPhase::WaitingAddressed => {
                        self.addressing_port = Some(port.port_num());
                        self.port_config_phase[port.port_num() as usize] =
                            ConfigPhase::ResettingPort;
                        port.reset();
                    }
                    _ => return Err(mkerror!(ErrorType::InvalidPhase)),
                },
            }
        }
        Ok(())
    }

    fn request_hc_ownership(&mut self) {
        if let Some(list) = self.registers.extended_register_list {
            for cap in list {
                match cap {
                    ExtendedCapability::UsbLegacySupport(mut reg) => {
                        let reg = reg.as_mut();
                        reg.update(|r| r.set_hc_os_owned_semaphore(true));
                        while reg.read().hc_bios_owned_semaphore()
                            || !reg.read().hc_os_owned_semaphore()
                        {}
                    }
                    _ => {
                        debug!("Unsupported extended capability");
                    }
                }
            }
        }
        debug!("Done requesting HC ownership");
    }

    fn initialize_host_controller(&mut self) {
        let operational = self.registers.operational.as_mut();
        let hc_halted = operational.usbsts.read().host_controller_halted();
        operational.usbcmd.update(|u| {
            u.set_interrupter_enable(false);
            u.set_host_system_error_enable(false);
            u.set_enable_wrap_event(false);
            if !hc_halted {
                u.set_run_stop(false);
            }
        });
        while !operational.usbsts.read().host_controller_halted() {}

        operational
            .usbcmd
            .update(|u| u.set_host_controller_reset(true));
        while operational.usbcmd.read().host_controller_reset() {}
        while operational.usbsts.read().controller_not_ready() {}
    }

    fn set_enabled_device_slots(&mut self) {
        let num_device_slots = self
            .registers
            .capability
            .as_ref()
            .hcsparams1
            .read()
            .max_device_slots();
        debug!("Max device slots: {}", num_device_slots);

        let max_slots = self.device_manager.max_slots() as u8;
        self.registers
            .operational
            .as_mut()
            .config
            .update(|c| c.set_max_device_slots_enabled(max_slots));
    }

    fn set_dcbaap(&mut self) {
        let ptr = self.device_manager.dcbaa_ptr();
        debug!("DCBAA ptr: 0x{:x}", ptr);
        self.registers
            .operational
            .as_mut()
            .dcbaap
            .update(|d| d.set_device_context_base_address_array_pointer(ptr));
    }

    fn init_command_ring(&mut self) {
        self.command_ring
            .initialize(32)
            .expect("Failed to initialize command ring");
        let ptr = self.command_ring.buffer_pointer();
        self.registers.operational.as_mut().crcr.update(|c| {
            c.set_ring_cycle_state(true);
            c.set_command_stop(false);
            c.set_command_abort(false);
            c.set_command_ring_pointer(ptr);
        });
    }

    fn init_event_ring(&mut self) {
        self.event_ring
            .initialize(32, self.registers.interrupter_register_set.at(0).unwrap())
            .expect("Failed to initialize event ring");
    }

    fn init_interrupter(&mut self) {
        self.registers
            .interrupter_register_set
            .at(0)
            .unwrap()
            .as_mut()
            .iman
            .update(|iman| {
                iman.set_interrupt_pending(true);
                iman.set_interrupt_enable(true);
            });

        self.registers.operational.as_mut().usbcmd.update(|u| {
            u.set_interrupter_enable(true);
        });
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ConfigPhase {
    NotConnected,
    WaitingAddressed,
    ResettingPort,
    EnablingSlot,
    AddressingDevice,
    InitializingDevice,
    ConfiguringEndpoints,
    Configured,
}
