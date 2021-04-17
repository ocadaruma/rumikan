use crate::usb::IdentityMapper;
use xhci::accessor::Single;
use xhci::registers::PortRegisterSet;

pub struct Port {
    num: u8,
    portsc: Single<PortRegisterSet, IdentityMapper>,
}

impl Port {
    pub fn new(num: u8, portsc: Single<PortRegisterSet, IdentityMapper>) -> Port {
        Port { num, portsc }
    }

    pub fn port_num(&self) -> u8 {
        self.num
    }

    pub fn is_connected(&self) -> bool {
        self.portsc.read().portsc.current_connect_status()
    }

    pub fn is_enabled(&self) -> bool {
        self.portsc.read().portsc.port_enabled_disabled()
    }

    pub fn is_port_reset_changed(&self) -> bool {
        self.portsc.read().portsc.port_reset_changed()
    }

    pub fn reset(&mut self) {
        self.portsc.update(|p| {
            p.portsc.bit_and_assign(0x0e00c3e0);
            p.portsc.bit_or_assign(0x00020010);
        });
        while self.portsc.read().portsc.port_reset() {}
    }

    pub fn clear_port_reset_change(&mut self) {
        self.portsc.update(|p| {
            p.portsc.bit_and_assign(0x0e01c3e0);
            p.portsc.set_port_reset_changed(true);
        });
    }
}
