use bit_field::BitField;

use crate::error::ErrorContext;
use crate::interrupt::InterruptVector;
use crate::util::collection::ArrayVec;

#[derive(Debug)]
pub enum ErrorType {
    TooManyDevices,
    IndexOutOfRange,
    NotImplemented,
    NoPCIMSI,
}

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub struct Pci {
    devices: ArrayVec<Device, 32>,
}

#[allow(clippy::new_without_default)]
impl Pci {
    pub fn new() -> Pci {
        Pci {
            devices: ArrayVec::new(),
        }
    }

    pub fn devices(&self) -> &[Device] {
        self.devices.as_slice()
    }

    pub fn scan_all_bus(&mut self) -> Result<()> {
        let header_type = Self::read_header_type(0, 0, 0);
        if Self::is_single_function_device(header_type) {
            return self.scan_bus(0);
        }
        for function in (0u8..8).filter(|&function| Self::read_vendor_id(0, 0, function) != 0xffff)
        {
            self.scan_bus(function)?;
        }
        Ok(())
    }

    fn scan_bus(&mut self, bus: u8) -> Result<()> {
        for device in (0u8..32).filter(|&device| Self::read_vendor_id(bus, device, 0) != 0xffff) {
            self.scan_device(bus, device)?;
        }
        Ok(())
    }

    fn scan_device(&mut self, bus: u8, device: u8) -> Result<()> {
        let header_type = Self::read_header_type(bus, device, 0);
        if Self::is_single_function_device(header_type) {
            return self.scan_function(bus, device, 0);
        }
        for function in
            (0u8..8).filter(|&function| Self::read_vendor_id(bus, device, function) != 0xffff)
        {
            self.scan_function(bus, device, function)?;
        }
        Ok(())
    }

    fn scan_function(&mut self, bus: u8, device: u8, function: u8) -> Result<()> {
        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(bus, device, function, 0x08).value(),
        );
        let data = in32(CONFIG_DATA);
        let class_code = ClassCode {
            base: ((data >> 24) & 0xff) as u8,
            sub: ((data >> 16) & 0xff) as u8,
            interface: ((data >> 8) & 0xff) as u8,
        };

        let header_type = Self::read_header_type(bus, device, function);
        if self
            .devices
            .push(Device {
                bus,
                device,
                function,
                header_type,
                class_code,
            })
            .is_err()
        {
            return Err(mkerror!(ErrorType::TooManyDevices));
        }

        if class_code.base == 0x06 && class_code.sub == 0x04 {
            out32(
                CONFIG_ADDRESS,
                ConfigAddress::new(bus, device, function, 0x18).value(),
            );
            let bus_numbers = in32(CONFIG_DATA);
            let secondary_bus = ((bus_numbers >> 8) & 0xff) as u8;
            self.scan_bus(secondary_bus)
        } else {
            Ok(())
        }
    }

    fn read_header_type(bus: u8, device: u8, function: u8) -> u8 {
        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(bus, device, function, 0x0c).value(),
        );
        ((in32(CONFIG_DATA) >> 16) & 0xff) as u8
    }

    fn read_vendor_id(bus: u8, device: u8, function: u8) -> u16 {
        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(bus, device, function, 0x00).value(),
        );
        (in32(CONFIG_DATA) & 0xffff) as u16
    }

    fn is_single_function_device(header_type: u8) -> bool {
        (header_type & 0x80) == 0
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct Device {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub header_type: u8,
    pub class_code: ClassCode,
}

impl Device {
    pub fn read_vendor_id(&self) -> u16 {
        out32(CONFIG_ADDRESS, self.config_address(0x00).value());
        (in32(CONFIG_DATA) & 0xffff) as u16
    }

    pub fn read_bar(&self, index: u8) -> Result<usize> {
        if index >= 6 {
            return Err(mkerror!(ErrorType::IndexOutOfRange));
        }

        let addr = 0x10 + (4 * index);
        let bar = self.read_config_reg(addr);
        if (bar & 4) == 0 {
            return Ok(bar as usize);
        }
        if index >= 5 {
            return Err(mkerror!(ErrorType::IndexOutOfRange));
        }

        let bar_upper = self.read_config_reg(addr + 4);
        Ok(((bar_upper as usize) << 32) | (bar as usize))
    }

    pub fn switch_ehci2xhci_if_necessary(&self, pci: &Pci) {
        if self.read_vendor_id() == 0x8086
            && pci.devices().iter().any(|dev| {
                dev.class_code
                    == ClassCode {
                        base: 0x0c,
                        sub: 0x03,
                        interface: 0x20,
                    }
            })
        {
            let superspeed_ports = self.read_config_reg(0xdc);
            self.write_config_reg(0xd8, superspeed_ports);

            let ehci2xhci_ports = self.read_config_reg(0xd4);
            self.write_config_reg(0xd0, ehci2xhci_ports);
        }
    }

    pub fn configure_msi_fixed_destination(
        &self,
        apic_id: u8,
        trigger_mode: MSITriggerMode,
        delivery_mode: MSIDeliveryMode,
        vector: InterruptVector,
        num_vector_exponent: u32,
    ) -> Result<()> {
        let msg_addr: u32 = 0xfee00000 | ((apic_id as u32) << 12);
        let mut msg_data = ((delivery_mode as u32) << 8) | ((vector as u8) as u32);
        if trigger_mode == MSITriggerMode::Level {
            msg_data |= 0xc000;
        }
        self.configure_msi(msg_addr, msg_data, num_vector_exponent)
    }

    fn configure_msi(&self, msg_addr: u32, msg_data: u32, num_vector_exponent: u32) -> Result<()> {
        let mut cap_addr = (self.read_config_reg(0x34) & 0xff) as u8;

        let (mut msi_cap_addr, mut _msix_cap_addr) = (0u8, 0u8);
        while cap_addr != 0 {
            let header = CapabilityHeader {
                data: self.read_config_reg(cap_addr),
            };
            match header.cap_id() {
                CapabilityHeader::CAPABILITY_MSI => msi_cap_addr = cap_addr,
                CapabilityHeader::CAPABILITY_MSIX => _msix_cap_addr = cap_addr,
                _ => {}
            }
            cap_addr = header.next_ptr();
        }

        if msi_cap_addr != 0 {
            self.configure_msi_register(msi_cap_addr, msg_addr, msg_data, num_vector_exponent);
            Ok(())
        } else {
            Err(mkerror!(ErrorType::NoPCIMSI))
        }
    }

    fn configure_msi_register(
        &self,
        cap_addr: u8,
        msg_addr: u32,
        msg_data: u32,
        num_vector_exponent: u32,
    ) {
        let mut msi_cap = self.read_msi_capability(cap_addr);

        if msi_cap.header.multi_msg_capable() as u32 <= num_vector_exponent {
            msi_cap
                .header
                .set_multi_msg_enable(msi_cap.header.multi_msg_capable());
        } else {
            msi_cap
                .header
                .set_multi_msg_enable(num_vector_exponent as u8);
        }

        msi_cap.header.set_msi_enable(true);
        msi_cap.msg_addr = msg_addr;
        msi_cap.msg_data = msg_data;

        self.write_config_reg(cap_addr, msi_cap.header.data);
        self.write_config_reg(cap_addr + 4, msi_cap.msg_addr);

        let msg_data_addr = if msi_cap.header.addr_64_capable() {
            self.write_config_reg(cap_addr + 8, msi_cap.msg_upper_addr);
            cap_addr + 12
        } else {
            cap_addr + 8
        };
        self.write_config_reg(msg_data_addr, msi_cap.msg_data);

        if msi_cap.header.per_vector_mask_capable() {
            self.write_config_reg(msg_data_addr + 4, msi_cap.mask_bits);
            self.write_config_reg(msg_data_addr + 8, msi_cap.pending_bits);
        }
    }

    fn read_msi_capability(&self, cap_addr: u8) -> MSICapability {
        let header = self.read_capability_header(cap_addr);

        let (msg_upper_addr, msg_data_addr) = if header.addr_64_capable() {
            (self.read_config_reg(cap_addr + 8), cap_addr + 12)
        } else {
            (0, cap_addr + 8)
        };

        let (mask_bits, pending_bits) = if header.per_vector_mask_capable() {
            (
                self.read_config_reg(msg_data_addr + 4),
                self.read_config_reg(msg_data_addr + 8),
            )
        } else {
            (0, 0)
        };

        MSICapability {
            header,
            msg_addr: self.read_config_reg(cap_addr + 4),
            msg_upper_addr,
            msg_data: self.read_config_reg(msg_data_addr),
            mask_bits,
            pending_bits,
        }
    }

    fn read_capability_header(&self, addr: u8) -> CapabilityHeader {
        CapabilityHeader {
            data: self.read_config_reg(addr),
        }
    }

    fn config_address(&self, reg_addr: u8) -> ConfigAddress {
        ConfigAddress::new(self.bus, self.device, self.function, reg_addr)
    }

    fn read_config_reg(&self, reg_addr: u8) -> u32 {
        out32(CONFIG_ADDRESS, self.config_address(reg_addr).value());
        in32(CONFIG_DATA)
    }

    fn write_config_reg(&self, reg_addr: u8, value: u32) {
        out32(CONFIG_ADDRESS, self.config_address(reg_addr).value());
        out32(CONFIG_DATA, value);
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum MSIDeliveryMode {
    Fixed = 0b000,
}

#[repr(u8)]
#[derive(Debug, Eq, PartialEq)]
pub enum MSITriggerMode {
    Level = 1,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct CapabilityHeader {
    data: u32,
}

impl CapabilityHeader {
    const CAPABILITY_MSI: u8 = 0x05;
    const CAPABILITY_MSIX: u8 = 0x11;

    getbits!(pub cap_id: u8; data; 0; 8);
    getbits!(pub next_ptr: u8; data; 8; 8);
    setbit!(pub set_msi_enable; data; 16);
    getbits!(pub multi_msg_capable: u8; data; 17; 3);
    setbits!(pub set_multi_msg_enable: u8; data; 20; 3);
    getbit!(pub addr_64_capable; data; 23);
    getbit!(pub per_vector_mask_capable; data; 24);
}

#[repr(C)]
#[derive(Debug)]
pub struct MSICapability {
    pub header: CapabilityHeader,
    pub msg_addr: u32,
    pub msg_upper_addr: u32,
    pub msg_data: u32,
    pub mask_bits: u32,
    pub pending_bits: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ClassCode {
    pub base: u8,
    pub sub: u8,
    pub interface: u8,
}

impl ClassCode {
    pub fn value(&self) -> u32 {
        ((self.base as u32) << 24) | ((self.sub as u32) << 16) | ((self.interface as u32) << 8)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ConfigAddress(u32);

impl ConfigAddress {
    pub fn new(bus: u8, device: u8, function: u8, reg_addr: u8) -> ConfigAddress {
        let bits = 1u32 << 31
            | (bus as u32) << 16
            | (device as u32) << 11
            | (function as u32) << 8
            | (reg_addr as u32 & 0xfc);

        ConfigAddress(bits)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;

fn out32(addr: u16, data: u32) {
    unsafe {
        asm!(
        "out dx, eax",
        in("dx") addr, in("eax") data
        );
    }
}

fn in32(addr: u16) -> u32 {
    unsafe {
        let data: u32;
        asm!(
        "in eax, dx",
        out("eax") data, in("dx") addr
        );
        data
    }
}

#[cfg(test)]
mod tests {
    use crate::pci::ClassCode;

    #[test]
    fn class_code_equality() {
        let xhc_class_code = ClassCode {
            base: 0x0c,
            sub: 0x03,
            interface: 0x30,
        };
        assert_eq!(
            xhc_class_code,
            ClassCode {
                base: 0x0c,
                sub: 0x03,
                interface: 0x30
            }
        );
    }
}
