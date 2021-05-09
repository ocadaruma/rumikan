use bit_field::BitField;
use core::mem::size_of;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum DescriptorType {
    InterruptGate = 14,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct InterruptDescriptorAttribute {
    data: u16,
}

#[allow(clippy::new_without_default)]
impl InterruptDescriptorAttribute {
    withbits!(_with_descriptor_type: u8; data; 8; 4);
    withbits!(pub with_descriptor_privilege_level: u8; data; 13; 2);
    setbit!(set_present; data; 15);

    pub fn with_descriptor_type(self, descriptor_type: DescriptorType) -> Self {
        self._with_descriptor_type(descriptor_type as u8)
    }

    pub fn new() -> Self {
        let mut attr = Self { data: 0 };
        attr.set_present(true);
        attr
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InterruptDescriptor {
    offset_low: u16,
    segment_selector: u16,
    attr: InterruptDescriptorAttribute,
    offset_middle: u16,
    offset_high: u32,
    _reserved: u32,
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable {
    data: [InterruptDescriptor {
        offset_low: 0,
        segment_selector: 0,
        attr: InterruptDescriptorAttribute { data: 0 },
        offset_middle: 0,
        offset_high: 0,
        _reserved: 0,
    }; 256],
};

#[repr(transparent)]
#[derive(Debug)]
pub struct InterruptDescriptorTable {
    data: [InterruptDescriptor; 256],
}

impl InterruptDescriptorTable {
    pub fn get_mut() -> &'static mut Self {
        unsafe { &mut IDT }
    }

    pub fn set(&mut self, v: InterruptVector, attr: InterruptDescriptorAttribute, offset: u64) {
        let cs: u16;
        unsafe {
            asm!(
            "mov {:x}, cs",
            out(reg) cs
            );
        }

        let desc = &mut self.data[(v as u8) as usize];
        desc.attr = attr;
        desc.offset_low = (offset & 0xffff) as u16;
        desc.offset_middle = ((offset >> 16) & 0xffff) as u16;
        desc.offset_high = (offset >> 32) as u32;
        desc.segment_selector = cs;
    }

    pub fn load(&self) {
        let ptr: *const Self = self;
        let ptr = DescriptorTablePointer::new()
            .with_limit((size_of::<InterruptDescriptorTable>() - 1) as u16)
            .with_ptr(ptr as u64);
        debug!("loading IDT: {:?}", ptr);
        unsafe {
            asm!(
              "lidt [{}]",
              in(reg) &ptr
            );
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct DescriptorTablePointer {
    data: [u16; 10],
}

impl DescriptorTablePointer {
    fn new() -> Self {
        Self { data: [0; 10] }
    }

    fn with_limit(mut self, limit: u16) -> Self {
        self.data[0] = limit;
        self
    }

    fn with_ptr(mut self, ptr: u64) -> Self {
        self.data[1] = (ptr & 0xffff) as u16;
        self.data[2] = ((ptr >> 16) & 0xffff) as u16;
        self.data[3] = ((ptr >> 32) & 0xffff) as u16;
        self.data[4] = (ptr >> 48) as u16;
        self
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct InterruptFrame {
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum InterruptVector {
    XHCI = 0x40,
}

pub fn notify_end_interrupt() {
    let ptr: u64 = 0xfee000b0;
    unsafe { (ptr as *mut u32).write_volatile(0) }
}
