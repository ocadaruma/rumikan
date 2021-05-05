use bit_field::BitField;
use core::ptr::null_mut;

#[repr(C)]
#[derive(Debug)]
pub struct MemMapRegister<T>(T);
impl<T> MemMapRegister<T> {
    pub fn read(&self) -> T {
        let ptr = (self as *const Self) as *const T;
        unsafe { ptr.read_volatile() }
    }

    pub fn write(&mut self, value: T) {
        let ptr = (self as *mut Self) as *mut T;
        unsafe { ptr.write_volatile(value) };
    }

    pub fn update<F>(&mut self, op: F)
    where
        F: FnOnce(&mut T),
    {
        let mut value = self.read();
        op(&mut value);
        self.write(value);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Accessor<T> {
    ptr: *mut T,
}
impl<T> Accessor<T> {
    pub fn null() -> Self {
        Self { ptr: null_mut() }
    }

    pub fn as_ref(&self) -> &T {
        unsafe { &*self.ptr }
    }

    pub fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ArrayAccessor<T> {
    ptr: *mut T,
    len: usize,
}
impl<T> ArrayAccessor<T> {
    pub fn at(&self, idx: usize) -> Option<Accessor<T>> {
        if idx < self.len {
            Some(Accessor {
                ptr: unsafe { self.ptr.add(idx) },
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HCSPARAMS1 {
    data: u32,
}
impl HCSPARAMS1 {
    getbits!(pub max_device_slots: u8; data; 0; 8);
    getbits!(pub max_ports: u8; data; 24; 8);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HCSPARAMS2 {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HCSPARAMS3 {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HCCPARAMS1 {
    data: u32,
}
impl HCCPARAMS1 {
    getbits!(pub xhci_extended_capabilities_pointer: u16; data; 16; 16);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HCCPARAMS2 {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct DBOFF {
    data: u32,
}
impl DBOFF {
    getbits!(pub offset: u32; data; 2; 30);

    pub fn doorbell_array_offset(&self) -> u32 {
        self.offset() << 2
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RTSOFF {
    data: u32,
}
impl RTSOFF {
    getbits!(pub offset: u32; data; 5; 27);

    pub fn runtime_register_space_offset(&self) -> u32 {
        self.offset() << 5
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct CapabilityRegisters {
    pub caplength: MemMapRegister<u8>,
    _reserved: MemMapRegister<u8>,
    _hci_version: MemMapRegister<u16>,
    pub hcsparams1: MemMapRegister<HCSPARAMS1>,
    pub hcsparams2: MemMapRegister<HCSPARAMS2>,
    pub hcsparams3: MemMapRegister<HCSPARAMS3>,
    pub hccparams1: MemMapRegister<HCCPARAMS1>,
    pub dboff: MemMapRegister<DBOFF>,
    pub rtsoff: MemMapRegister<RTSOFF>,
    pub hccparams2: MemMapRegister<HCCPARAMS2>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct USBCMD {
    data: u32,
}
impl USBCMD {
    setbit!(pub set_run_stop; data; 0);
    getbit!(pub host_controller_reset; data; 1);
    setbit!(pub set_host_controller_reset; data; 1);
    setbit!(pub set_interrupter_enable; data; 2);
    setbit!(pub set_host_system_error_enable; data; 3);
    setbit!(pub set_enable_wrap_event; data; 10);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct USBSTS {
    data: u32,
}
impl USBSTS {
    getbit!(pub host_controller_halted; data; 0);
    getbit!(pub controller_not_ready; data; 11);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CRCR {
    data: u64,
}
impl CRCR {
    setbit!(pub set_ring_cycle_state; data; 0);
    setbit!(pub set_command_stop; data; 1);
    setbit!(pub set_command_abort; data; 2);
    setbits!(set_pointer: u64; data; 6; 58);

    pub fn set_command_ring_pointer(&mut self, ptr: u64) {
        self.set_pointer(ptr >> 6);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct DCBAAP {
    data: u64,
}
impl DCBAAP {
    setbits!(set_pointer: u64; data; 6; 58);

    pub fn set_device_context_base_address_array_pointer(&mut self, ptr: u64) {
        self.set_pointer(ptr >> 6);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CONFIG {
    data: u32,
}
impl CONFIG {
    setbits!(pub set_max_device_slots_enabled: u8; data; 0; 8);
}

#[repr(C)]
#[derive(Debug)]
pub struct OperationalRegisters {
    pub usbcmd: MemMapRegister<USBCMD>,
    pub usbsts: MemMapRegister<USBSTS>,
    _pagesize: MemMapRegister<u32>,
    _reserved1: [u32; 2],
    _dnctrl: MemMapRegister<u32>,
    pub crcr: MemMapRegister<CRCR>,
    _reserved2: [u32; 4],
    pub dcbaap: MemMapRegister<DCBAAP>,
    pub config: MemMapRegister<CONFIG>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PORTSC {
    data: u32,
}
impl PORTSC {
    getbit!(pub current_connect_status; data; 0);
    getbit!(pub port_enabled_disabled; data; 1);
    getbit!(pub port_reset; data; 4);
    getbits!(pub port_speed: u8; data; 10; 4);
    getbit!(pub port_reset_change; data; 21);
    setbit!(set_port_reset_change; data; 21);

    pub fn reset(&mut self) {
        self.data &= 0x0e00c3e0;
        self.data |= 0x00020010;
    }

    pub fn clear_status_bit(&mut self) {
        self.data &= 0x0e01c3e0;
        self.set_port_reset_change(true);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PORTPMSC {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PORTLI {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PORTHLPMC {
    data: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct PortRegisterSet {
    pub portsc: MemMapRegister<PORTSC>,
    _portpmsc: MemMapRegister<PORTPMSC>,
    _portli: MemMapRegister<PORTLI>,
    _porthlpmc: MemMapRegister<PORTHLPMC>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct IMAN {
    data: u32,
}
impl IMAN {
    setbit!(pub set_interrupt_pending; data; 0);
    setbit!(pub set_interrupt_enable; data; 1);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct IMOD {
    data: u32,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ERSTSZ {
    data: u32,
}
impl ERSTSZ {
    setbits!(pub set_event_ring_segment_table_size: u16; data; 0; 16);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ERSTBA {
    data: u64,
}
impl ERSTBA {
    setbits!(set_erstba: u64; data; 6; 58);

    pub fn set_event_ring_segment_table_base_address(&mut self, ptr: u64) {
        self.set_erstba(ptr >> 6);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ERDP {
    data: u64,
}
impl ERDP {
    getbits!(erdp: u64; data; 4; 60);
    setbits!(set_erdp: u64; data; 4; 60);

    pub fn event_ring_dequeue_pointer(&self) -> u64 {
        self.erdp() << 4
    }

    pub fn set_event_ring_dequeue_pointer(&mut self, ptr: u64) {
        self.set_erdp(ptr >> 4);
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct InterrupterRegisterSet {
    pub iman: MemMapRegister<IMAN>,
    pub imod: MemMapRegister<IMOD>,
    pub erstsz: MemMapRegister<ERSTSZ>,
    _reserved: u32,
    pub erstba: MemMapRegister<ERSTBA>,
    pub erdp: MemMapRegister<ERDP>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Doorbell {
    data: u32,
}
impl Doorbell {
    setbits!(pub set_db_target: u8; data; 0; 8);
    setbits!(pub set_db_stream_id: u16; data; 16; 16);
}

#[repr(C)]
#[derive(Debug)]
pub struct DoorbellRegister {
    reg: MemMapRegister<Doorbell>,
}
impl DoorbellRegister {
    pub fn ring(&mut self, target: u8, stream_id: u16) {
        self.reg.update(|r| {
            r.set_db_target(target);
            r.set_db_stream_id(stream_id);
        });
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ExtendedRegister {
    data: u32,
}
impl ExtendedRegister {
    getbits!(pub capability_id: u8; data; 0; 8);
    getbits!(pub next_pointer: u8; data; 8; 8);
}

#[derive(Debug)]
#[repr(transparent)]
pub struct USBLEGSUP {
    data: u32,
}
impl USBLEGSUP {
    getbit!(pub hc_bios_owned_semaphore; data; 16);
    getbit!(pub hc_os_owned_semaphore; data; 24);
    setbit!(pub set_hc_os_owned_semaphore; data; 24);
}

#[derive(Debug)]
pub enum ExtendedCapability {
    Unsupported,
    UsbLegacySupport(Accessor<MemMapRegister<USBLEGSUP>>),
}

#[derive(Debug, Copy, Clone)]
pub struct ExtendedRegisterList {
    ptr: *mut MemMapRegister<ExtendedRegister>,
}

impl IntoIterator for ExtendedRegisterList {
    type Item = ExtendedCapability;
    type IntoIter = IterMutExt;

    fn into_iter(self) -> Self::IntoIter {
        IterMutExt(self.ptr)
    }
}

#[derive(Debug)]
pub struct IterMutExt(*mut MemMapRegister<ExtendedRegister>);
impl Iterator for IterMutExt {
    type Item = ExtendedCapability;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            None
        } else {
            let reg = unsafe { &*self.0 };
            let result = if reg.read().capability_id() == 1 {
                ExtendedCapability::UsbLegacySupport(Accessor {
                    ptr: self.0 as *mut MemMapRegister<USBLEGSUP>,
                })
            } else {
                ExtendedCapability::Unsupported
            };
            if reg.read().next_pointer() == 0 {
                self.0 = null_mut();
            } else {
                self.0 = unsafe { self.0.add(reg.read().next_pointer() as usize) };
            }
            Some(result)
        }
    }
}

#[derive(Debug)]
pub struct Registers {
    pub capability: Accessor<CapabilityRegisters>,
    pub operational: Accessor<OperationalRegisters>,
    pub doorbell: ArrayAccessor<DoorbellRegister>,
    pub port_register_set: ArrayAccessor<PortRegisterSet>,
    pub interrupter_register_set: ArrayAccessor<InterrupterRegisterSet>,
    pub extended_register_list: Option<ExtendedRegisterList>,
}

impl Registers {
    pub fn new(mmio_base: usize) -> Self {
        let cap = Accessor {
            ptr: mmio_base as *mut CapabilityRegisters,
        };
        let operational_base = mmio_base + cap.as_ref().caplength.read() as usize;
        let doorbell_base = mmio_base + cap.as_ref().dboff.read().doorbell_array_offset() as usize;
        let interrupter_register_set_base = mmio_base
            + (cap.as_ref().rtsoff.read().runtime_register_space_offset() as usize)
            + 0x20;
        let port_register_set_base = operational_base + 0x400;
        let max_ports = cap.as_ref().hcsparams1.read().max_ports();

        let ext_ptr = cap
            .as_ref()
            .hccparams1
            .read()
            .xhci_extended_capabilities_pointer();
        let extended_register_list_base = if ext_ptr == 0 {
            None
        } else {
            Some(mmio_base + ext_ptr as usize)
        };

        Self {
            capability: cap,
            operational: Accessor {
                ptr: operational_base as *mut OperationalRegisters,
            },
            doorbell: ArrayAccessor {
                ptr: doorbell_base as *mut DoorbellRegister,
                len: 256,
            },
            port_register_set: ArrayAccessor {
                ptr: port_register_set_base as *mut PortRegisterSet,
                len: max_ports as usize,
            },
            interrupter_register_set: ArrayAccessor {
                ptr: interrupter_register_set_base as *mut InterrupterRegisterSet,
                len: 1024,
            },
            extended_register_list: extended_register_list_base.map(|base| ExtendedRegisterList {
                ptr: base as *mut MemMapRegister<ExtendedRegister>,
            }),
        }
    }
}
