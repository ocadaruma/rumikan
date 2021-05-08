use crate::error::ErrorContext;
use crate::util::collection::ArrayVec;

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
}

#[derive(Debug)]
pub enum ErrorType {
    TooManyDevices,
    IndexOutOfRange,
}

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

impl Pci {
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
        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(self.bus, self.device, self.function, 0x00).value(),
        );
        (in32(CONFIG_DATA) & 0xffff) as u16
    }

    pub fn read_bar(&self, index: u8) -> Result<usize> {
        if index >= 6 {
            return Err(mkerror!(ErrorType::IndexOutOfRange));
        }

        let addr = 0x10 + (4 * index);
        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(self.bus, self.device, self.function, addr).value(),
        );
        let bar = in32(CONFIG_DATA);
        if (bar & 4) == 0 {
            return Ok(bar as usize);
        }
        if index >= 5 {
            return Err(mkerror!(ErrorType::IndexOutOfRange));
        }

        out32(
            CONFIG_ADDRESS,
            ConfigAddress::new(self.bus, self.device, self.function, addr + 4).value(),
        );
        let bar_upper = in32(CONFIG_DATA);
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
            out32(CONFIG_ADDRESS, 0xdc);
            let superspeed_ports = in32(CONFIG_DATA);
            out32(CONFIG_ADDRESS, 0xd8);
            out32(CONFIG_DATA, superspeed_ports);

            out32(CONFIG_ADDRESS, 0xd4);
            let ehci2xhci_ports = in32(CONFIG_DATA);
            out32(CONFIG_ADDRESS, 0xd0);
            out32(CONFIG_DATA, ehci2xhci_ports);
        }
    }
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
