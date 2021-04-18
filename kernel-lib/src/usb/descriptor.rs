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
            _ => DescriptorType::Unsupported,
        }
    }
}

#[derive(Debug)]
pub enum DescriptorType {
    Unsupported,
    Device(DeviceDescriptor),
    Configuration(ConfigurationDescriptor),
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
}
