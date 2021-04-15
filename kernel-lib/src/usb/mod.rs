mod mem;
mod ring;

use crate::usb::mem::allocate;
use crate::usb::ring::{EventRing, Ring};
use core::num::NonZeroUsize;
use xhci::accessor::Mapper;
use xhci::{ExtendedCapability, Registers};

const NUM_DEVICE_SLOTS: u8 = 8;

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

pub struct Xhc {
    registers: Registers<IdentityMapper>,
    extended_capabilities: xhci::extended_capabilities::List<IdentityMapper>,
    command_ring: Ring,
    event_ring: EventRing,
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
            command_ring: Ring::default(),
            event_ring: EventRing::default(),
        }
    }

    pub fn initialize(&mut self) {
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

    // pub fn process_event(&mut self) {
    //
    // }

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
        printk!("Max device slots: {}\n", num_device_slots);
        self.registers
            .operational
            .config
            .update(|c| c.set_max_device_slots_enabled(NUM_DEVICE_SLOTS));
    }

    fn set_dcbaap(&mut self) {
        let device_context_size: usize =
            if self.registers.capability.hccparams1.read().context_size() {
                2048
            } else {
                1024
            };
        let ptr = allocate::<()>(
            device_context_size * (NUM_DEVICE_SLOTS as usize + 1),
            Some(64),
            Some(4096),
        )
        .expect("Not enough memory");
        self.registers
            .operational
            .dcbaap
            .update(|d| d.set(ptr as u64));
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
            .initialize(
                32,
                self.registers
                    .interrupt_register_set
                    .as_single(0, IdentityMapper),
            )
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
