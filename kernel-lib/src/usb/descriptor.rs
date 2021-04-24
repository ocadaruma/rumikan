use core::mem::transmute;
use core::slice::from_raw_parts;

#[derive(Debug)]
pub struct Descriptor(*const u8);
impl Descriptor {
    pub fn new(buf: *const u8) -> Descriptor {
        Descriptor(buf)
    }

    pub fn specialize(&self) -> DescriptorType {
        match unsafe { self.0.add(1).read() } {
            DeviceDescriptor::TYPE => DescriptorType::Device(unsafe {
                transmute::<_, *const DeviceDescriptor>(self.0).read()
            }),
            ConfigurationDescriptor::TYPE => DescriptorType::Configuration(unsafe {
                transmute::<_, *const ConfigurationDescriptor>(self.0).read()
            }),
            InterfaceDescriptor::TYPE => DescriptorType::Interface(unsafe {
                transmute::<_, *const InterfaceDescriptor>(self.0).read()
            }),
            EndpointDescriptor::TYPE => DescriptorType::Endpoint(unsafe {
                transmute::<_, *const EndpointDescriptor>(self.0).read()
            }),
            HidDescriptor::TYPE => {
                DescriptorType::Hid(unsafe { transmute::<_, *const HidDescriptor>(self.0).read() })
            }
            _ => DescriptorType::Unsupported,
        }
    }

    pub fn iter(&self, len: usize) -> DescriptorIter {
        DescriptorIter { ptr: self.0, len }
    }
}

pub struct DescriptorIter {
    ptr: *const u8,
    len: usize,
}
impl Iterator for DescriptorIter {
    type Item = DescriptorType;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
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

    pub fn num_endpoints(&self) -> u8 {
        self.0[4]
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct EndpointDescriptor([u8; 7]);
impl EndpointDescriptor {
    pub const TYPE: u8 = 5;
}

#[repr(transparent)]
#[derive(Debug)]
pub struct HidDescriptor([u8; 6]);
impl HidDescriptor {
    pub const TYPE: u8 = 33;
}
