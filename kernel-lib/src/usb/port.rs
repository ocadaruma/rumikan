use crate::usb::endpoint::EndpointType;
use crate::usb::IdentityMapper;
use core::convert::TryFrom;
use xhci::accessor::Single;
use xhci::registers::PortRegisterSet;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum PortSpeed {
    FullSpeed = PortSpeed::FULL_SPEED,
    LowSpeed = PortSpeed::LOW_SPEED,
    _HighSpeed = PortSpeed::HIGH_SPEED,
    _SuperSpeed = PortSpeed::SUPER_SPEED,
    _SuperSpeedPlus = PortSpeed::SUPER_SPEED_PLUS,
}

impl PortSpeed {
    const FULL_SPEED: u8 = 1;
    const LOW_SPEED: u8 = 2;
    const HIGH_SPEED: u8 = 3;
    const SUPER_SPEED: u8 = 4;
    const SUPER_SPEED_PLUS: u8 = 5;

    pub fn convert_interval(&self, endpoint_type: EndpointType, interval: u32) -> u32 {
        match &self {
            PortSpeed::FullSpeed | PortSpeed::LowSpeed => match endpoint_type {
                EndpointType::Isochronous => interval * 2,
                _ => (msb1(interval).map(|i| i as i32).unwrap_or(-1) + 3) as u32,
            },
            _ => interval - 1,
        }
    }
}

fn msb1(n: u32) -> Option<u8> {
    let mut n = n.reverse_bits();
    let mut msb: Option<u8> = None;
    let mut i = 0x1f;
    while n > 0 {
        if n & 1 == 1 {
            msb = Some(i);
            break;
        }
        n >>= 1;
        i -= 1;
    }
    msb
}

impl TryFrom<u8> for PortSpeed {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            PortSpeed::FULL_SPEED => Ok(PortSpeed::FullSpeed),
            PortSpeed::LOW_SPEED => Ok(PortSpeed::LowSpeed),
            PortSpeed::HIGH_SPEED => Ok(PortSpeed::_HighSpeed),
            PortSpeed::SUPER_SPEED => Ok(PortSpeed::_SuperSpeed),
            PortSpeed::SUPER_SPEED_PLUS => Ok(PortSpeed::_SuperSpeedPlus),
            _ => Err(()),
        }
    }
}

pub struct Port {
    num: u8,
    reg: Single<PortRegisterSet, IdentityMapper>,
}

impl Port {
    pub fn new(num: u8, reg: Single<PortRegisterSet, IdentityMapper>) -> Port {
        Port { num, reg }
    }

    pub fn port_num(&self) -> u8 {
        self.num
    }

    pub fn is_connected(&self) -> bool {
        self.reg.read().portsc.current_connect_status()
    }

    pub fn is_enabled(&self) -> bool {
        self.reg.read().portsc.port_enabled_disabled()
    }

    pub fn is_port_reset_changed(&self) -> bool {
        self.reg.read().portsc.port_reset_changed()
    }

    pub fn port_speed(&self) -> Option<PortSpeed> {
        PortSpeed::try_from(self.reg.read().portsc.port_speed()).ok()
    }

    pub fn reset(&mut self) {
        self.reg.update(|p| {
            p.portsc.bit_and_assign(0x0e00c3e0);
            p.portsc.bit_or_assign(0x00020010);
        });
        while self.reg.read().portsc.port_reset() {}
    }

    pub fn clear_port_reset_change(&mut self) {
        self.reg.update(|p| {
            p.portsc.bit_and_assign(0x0e01c3e0);
            p.portsc.set_port_reset_changed(true);
        });
    }
}

mod tests {
    use crate::usb::port::msb1;

    #[test]
    fn test_msb1() {
        assert_eq!(msb1(0), None);
        assert_eq!(msb1(1), Some(0));
        assert_eq!(msb1(2), Some(1));
        assert_eq!(msb1(16), Some(4));
        assert_eq!(msb1(31), Some(4));
        assert_eq!(msb1(u32::MAX), Some(31));
    }
}
