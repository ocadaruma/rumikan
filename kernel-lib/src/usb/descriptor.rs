use bit_field::BitField;

#[derive(Debug)]
pub struct Descriptor(*const u8);
impl Descriptor {
    pub fn new(buf: *const u8) -> Descriptor {
        Descriptor(buf)
    }

    pub fn specialize(&self) -> DescriptorType {
        match unsafe { self.0.add(1).read() } {
            DeviceDescriptor::TYPE => {
                DescriptorType::Device(unsafe { (self.0 as *const DeviceDescriptor).read() })
            }
            ConfigurationDescriptor::TYPE => DescriptorType::Configuration(unsafe {
                (self.0 as *const ConfigurationDescriptor).read()
            }),
            InterfaceDescriptor::TYPE => {
                DescriptorType::Interface(unsafe { (self.0 as *const InterfaceDescriptor).read() })
            }
            EndpointDescriptor::TYPE => {
                DescriptorType::Endpoint(unsafe { (self.0 as *const EndpointDescriptor).read() })
            }
            HidDescriptor::TYPE => {
                DescriptorType::Hid(unsafe { (self.0 as *const HidDescriptor).read() })
            }
            _ => DescriptorType::Unsupported,
        }
    }

    pub fn iter(&self, len: usize) -> DescriptorIter {
        DescriptorIter {
            ptr: self.0,
            len,
            offset: 0,
        }
    }
}

pub struct DescriptorIter {
    ptr: *const u8,
    len: usize,
    offset: usize,
}
impl Iterator for DescriptorIter {
    type Item = DescriptorType;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.len {
            let ptr = self.ptr;
            let desc_len = unsafe { *ptr } as usize;

            self.offset += desc_len;
            self.ptr = unsafe { ptr.add(desc_len) };

            Some(Descriptor(self.ptr).specialize())
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum DescriptorType {
    Unsupported,
    Device(DeviceDescriptor),
    Configuration(ConfigurationDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
    Hid(HidDescriptor),
}

#[repr(transparent)]
#[derive(Debug)]
pub struct DeviceDescriptor([u8; 18]);
impl DeviceDescriptor {
    pub const TYPE: u8 = 1;
}

#[repr(transparent)]
#[derive(Debug)]
pub struct ConfigurationDescriptor([u8; 9]);
impl ConfigurationDescriptor {
    pub const TYPE: u8 = 2;

    pub fn configuration_value(&self) -> u8 {
        self.0[5]
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct InterfaceDescriptor([u8; 9]);
impl InterfaceDescriptor {
    pub const TYPE: u8 = 4;

    pub fn interface_number(&self) -> u8 {
        self.0[2]
    }

    pub fn num_endpoints(&self) -> u8 {
        self.0[4]
    }

    pub fn interface_class(&self) -> u8 {
        self.0[5]
    }

    pub fn interface_sub_class(&self) -> u8 {
        self.0[6]
    }

    pub fn interface_protocol(&self) -> u8 {
        self.0[7]
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct EndpointDescriptor {
    _length: u8,
    _descriptor_type: u8,
    endpoint_address: u8,
    attributes: u8,
    max_packet_size: u16,
    interval: u8,
}
impl EndpointDescriptor {
    pub const TYPE: u8 = 5;

    pub fn endpoint_address_number(&self) -> u8 {
        self.endpoint_address.get_bits(0..4)
    }

    pub fn endpoint_address_dir_in(&self) -> bool {
        self.endpoint_address.get_bit(7)
    }

    pub fn attributes_transfer_type(&self) -> u8 {
        self.attributes.get_bits(0..2)
    }

    pub fn max_packet_size(&self) -> u16 {
        self.max_packet_size
    }

    pub fn interval(&self) -> u8 {
        self.interval
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct HidDescriptor([u8; 6]);
impl HidDescriptor {
    pub const TYPE: u8 = 33;
}
