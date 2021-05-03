mod classdriver;
mod context;
mod descriptor;
mod devmgr;
mod endpoint;
mod mem;
mod port;
mod ring;

use crate::error::ErrorContext;
use crate::usb::devmgr::DeviceManager;
use crate::usb::port::Port;
use crate::usb::ring::{
    AddressDeviceCommandTrb, CommandCompletionEventTrb, ConfigureEndpointCommandTrb,
    EnableSlotCommandTrb, EventRing, PortStatusChangeEventTrb, Ring, TransferEventTrb, TrbType,
};
use core::num::NonZeroUsize;
use xhci::accessor::Mapper;
use xhci::{ExtendedCapability, Registers};

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

#[derive(Copy, Clone, Debug)]
pub struct IdentityMapper;

impl Mapper for IdentityMapper {
    unsafe fn map(&mut self, phys_start: usize, _bytes: usize) -> NonZeroUsize {
        NonZeroUsize::new_unchecked(phys_start)
    }

    fn unmap(&mut self, _virt_start: usize, _bytes: usize) {
        // noop
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
    registers: Registers<IdentityMapper>,
    extended_capabilities: xhci::extended_capabilities::List<IdentityMapper>,
    device_manager: DeviceManager,
    command_ring: Ring,
    event_ring: EventRing,
    port_config_phase: [ConfigPhase; 256],
    addressing_port: Option<u8>,
}

impl Xhc {
    pub fn new(mmio_base: usize) -> Xhc {
        let mapper = IdentityMapper;
        let registers = unsafe { Registers::new(mmio_base, mapper) };
        let extended_capabilities = unsafe {
            xhci::extended_capabilities::List::new(
                mmio_base,
                registers.capability.hccparams1.read(),
                mapper,
            )
        }
        .unwrap();
        Xhc {
            registers,
            extended_capabilities,
            device_manager: DeviceManager::new(),
            command_ring: Ring::default(),
            event_ring: EventRing::default(),
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
        self.registers.operational.usbcmd.update(|u| {
            u.set_run_stop(true);
        });

        while self.registers.operational.usbsts.read().hc_halted() {}
    }

    pub fn max_ports(&self) -> u8 {
        self.registers
            .capability
            .hcsparams1
            .read()
            .number_of_ports()
    }

    pub fn port_at(&self, num: u8) -> Port {
        Port::new(
            num,
            self.registers.port_register_set.single_at(num as usize - 1),
        )
    }

    pub fn configure_port(&mut self, port: &mut Port) -> Result<()> {
        if self.port_config_phase[port.port_num() as usize] == ConfigPhase::NotConnected {
            self.reset_port(port)
        } else {
            Ok(())
        }
    }

    pub fn process_event(&mut self) -> Result<()> {
        match self.event_ring.peek_front() {
            Some(trb) => {
                let result = match trb.specialize() {
                    TrbType::TransferEvent(trb) => self.on_transfer_event(trb),
                    TrbType::CommandCompletionEvent(trb) => self.on_command_completion_event(trb),
                    TrbType::PortStatusChangeEvent(trb) => self.on_port_status_change_event(trb),
                    other => {
                        debug!("Unexpected trb type: {:?}", other);
                        Err(make_error!(ErrorType::NotImplemented))
                    }
                };
                self.event_ring.pop();
                result
            }
            None => Ok(()),
        }
    }

    fn on_transfer_event(&mut self, trb: TransferEventTrb) -> Result<()> {
        let slot_id = trb.slot_id();
        debug!("TransferEvent: slot_id = {}", slot_id.value());

        let dev = self
            .device_manager
            .find_by_slot(slot_id)
            .ok_or_else(|| make_error!(ErrorType::InvalidSlotId))?;
        dev.on_transfer_event_received(&trb)
            .map_err(ErrorType::DeviceError)
            .map_err(|e| make_error!(e))?;

        let port_id = dev.device_context().slot_context.root_hub_port_num();
        if dev.is_initialized()
            && self.port_config_phase[port_id as usize] == ConfigPhase::InitializingDevice
        {
            self.configure_endpoints(slot_id, port_id)
        } else {
            Ok(())
        }
    }

    fn on_command_completion_event(&mut self, trb: CommandCompletionEventTrb) -> Result<()> {
        debug!(
            "CommandCompletionEvent: slot_id = {}",
            trb.slot_id().value()
        );

        match unsafe { *trb.trb_pointer() }.specialize() {
            TrbType::EnableSlotCommand(_) => {
                if let Some(addressing_port) = self.addressing_port {
                    if self.port_config_phase[addressing_port as usize] == ConfigPhase::EnablingSlot
                    {
                        return self.address_device(addressing_port, trb.slot_id());
                    }
                }
            }
            TrbType::AddressDeviceCommand(_) => {
                let port_id = {
                    let dev = self
                        .device_manager
                        .find_by_slot(trb.slot_id())
                        .ok_or_else(|| make_error!(ErrorType::InvalidSlotId))?;
                    dev.device_context().slot_context.root_hub_port_num()
                };

                if self.addressing_port == Some(port_id)
                    && self.port_config_phase[port_id as usize] == ConfigPhase::AddressingDevice
                {
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
                        .map_err(ErrorType::DeviceError)
                        .map_err(|e| make_error!(e));
                }
            }
            TrbType::ConfigureEndpointCommand(_) => {
                let dev = self
                    .device_manager
                    .find_by_slot(trb.slot_id())
                    .ok_or_else(|| make_error!(ErrorType::InvalidSlotId))?;
                let port_id = dev.device_context().slot_context.root_hub_port_num();
                if self.port_config_phase[port_id as usize] == ConfigPhase::ConfiguringEndpoints {
                    self.port_config_phase[port_id as usize] = ConfigPhase::Configured;
                    return dev
                        .on_endpoints_configured()
                        .map_err(ErrorType::DeviceError)
                        .map_err(|e| make_error!(e));
                }
            }
            _ => {}
        }
        Err(make_error!(ErrorType::InvalidPhase))
    }

    fn address_device(&mut self, port_id: u8, slot_id: SlotId) -> Result<()> {
        let port = self.port_at(port_id);
        let dbreg = self.registers.doorbell.single_at(slot_id.value() as usize);
        self.device_manager
            .allocate_device(slot_id, dbreg)
            .map_err(ErrorType::DeviceError)
            .map_err(|e| make_error!(e))?;

        let dev = self
            .device_manager
            .find_by_slot(slot_id)
            .expect("Existence is guaranteed here");
        dev.address_device(port)
            .map_err(ErrorType::DeviceError)
            .map_err(|e| make_error!(e))?;

        self.port_config_phase[port_id as usize] = ConfigPhase::AddressingDevice;
        let cmd = AddressDeviceCommandTrb::new(slot_id, dev.input_context_ptr());

        self.command_ring.push(cmd.data());
        self.registers.doorbell.update_at(0, |d| {
            d.set_doorbell_target(0);
            d.set_doorbell_stream_id(0);
        });

        Ok(())
    }

    fn on_port_status_change_event(&mut self, trb: PortStatusChangeEventTrb) -> Result<()> {
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
                Err(make_error!(ErrorType::InvalidPhase))
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
            .map_err(ErrorType::DeviceError)
            .map_err(|e| make_error!(e))?;

        self.port_config_phase[port_id as usize] = ConfigPhase::ConfiguringEndpoints;
        let cmd = ConfigureEndpointCommandTrb::new(slot_id, input_context_ptr);

        self.command_ring.push(cmd.data());
        self.registers.doorbell.update_at(0, |d| {
            d.set_doorbell_target(0);
            d.set_doorbell_stream_id(0);
        });

        Ok(())
    }

    fn enable_slot(&mut self, port: &mut Port) {
        if port.is_enabled() && port.is_port_reset_changed() {
            port.clear_port_reset_change();
            self.port_config_phase[port.port_num() as usize] = ConfigPhase::EnablingSlot;

            let trb = EnableSlotCommandTrb::default();
            self.command_ring.push(trb.data());

            self.registers.doorbell.update_at(0, |d| {
                d.set_doorbell_target(0);
                d.set_doorbell_stream_id(0);
            });
        }
    }

    fn reset_port(&mut self, port: &mut Port) -> Result<()> {
        if port.is_connected() {
            match self.addressing_port {
                Some(addressing_port) => {
                    debug!("Setting port to waiting. port: {}", addressing_port);
                    self.port_config_phase[port.port_num() as usize] =
                        ConfigPhase::WaitingAddressed;
                }
                None => match self.port_config_phase[port.port_num() as usize] {
                    ConfigPhase::NotConnected | ConfigPhase::WaitingAddressed => {
                        debug!("Setting port to resetting. port: {}", port.port_num());
                        self.addressing_port = Some(port.port_num());
                        self.port_config_phase[port.port_num() as usize] =
                            ConfigPhase::ResettingPort;
                        port.reset();
                    }
                    _ => return Err(make_error!(ErrorType::InvalidPhase)),
                },
            }
        }
        Ok(())
    }

    fn request_hc_ownership(&mut self) {
        for cap in self.extended_capabilities.into_iter().flatten() {
            if let ExtendedCapability::UsbLegacySupportCapability(mut u) = cap {
                u.update(|s| s.set_hc_os_owned_semaphore(true));

                while u.read().hc_bios_owned_semaphore() || !u.read().hc_os_owned_semaphore() {}
            }
        }
    }

    fn initialize_host_controller(&mut self) {
        self.registers
            .operational
            .usbcmd
            .update(|u| u.set_run_stop(false));
        while !self.registers.operational.usbsts.read().hc_halted() {}

        self.registers
            .operational
            .usbcmd
            .update(|u| u.set_host_controller_reset(true));
        while self
            .registers
            .operational
            .usbcmd
            .read()
            .host_controller_reset()
        {}
        while self
            .registers
            .operational
            .usbsts
            .read()
            .controller_not_ready()
        {}
    }

    fn set_enabled_device_slots(&mut self) {
        let num_device_slots = self
            .registers
            .capability
            .hcsparams1
            .read()
            .number_of_device_slots();
        debug!("Max device slots: {}", num_device_slots);

        let max_slots = self.device_manager.max_slots() as u8;
        self.registers
            .operational
            .config
            .update(|c| c.set_max_device_slots_enabled(max_slots));
    }

    fn set_dcbaap(&mut self) {
        let ptr = self.device_manager.dcbaa_ptr();
        self.registers.operational.dcbaap.update(|d| d.set(ptr));
    }

    fn init_command_ring(&mut self) {
        self.command_ring
            .initialize(32)
            .expect("Failed to initialize command ring");
        let ptr = self.command_ring.ptr_as_u64();
        self.registers.operational.crcr.update(|c| {
            c.set_ring_cycle_state(true);
            c.set_command_stop(false);
            c.set_command_abort(false);
            c.set_command_ring_pointer(ptr);
        });
    }

    fn init_event_ring(&mut self) {
        self.event_ring
            .initialize(32, self.registers.interrupt_register_set.single_at(0))
            .expect("Failed to initialize event ring");
    }

    fn init_interrupter(&mut self) {
        self.registers
            .interrupt_register_set
            .update_at(0, |primary_interrupter| {
                primary_interrupter.iman.set_interrupt_pending(true);
                primary_interrupter.iman.set_interrupt_enable(true);
            });

        self.registers.operational.usbcmd.update(|u| {
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
