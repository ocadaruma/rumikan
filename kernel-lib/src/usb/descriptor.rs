use core::mem::transmute;
use core::slice::from_raw_parts;

#[derive(Debug)]
pub struct Descriptor(*const u8);
impl Descriptor {
    pub fn specialize(&self) -> DescriptorType {
        match unsafe { self.0.add(1).read() } {
            DeviceDescriptor::TYPE => DescriptorType::Device(DeviceDescriptor(unsafe {
                *transmute::<_, *const [u8; 18]>(self.0)
            })),
            ConfigurationDescriptor::TYPE => {
                DescriptorType::Configuration(ConfigurationDescriptor(unsafe {
                    *transmute::<_, *const [u8; 9]>(self.0)
                }))
            }
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
