#[macro_export]
macro_rules! getbit {
    ($vis:vis $name:ident ; $field:tt $([$idx:literal])? ; $offset:literal) => {
        $vis fn $name(&self) -> bool {
            (self.$field $([$idx])?).get_bit($offset)
        }
    };
}

#[macro_export]
macro_rules! getbits {
    ($vis:vis $name:ident : $ty:ty ; $field:tt $([$idx:literal])? ; $offset:literal ; $length:literal) => {
        $vis fn $name(&self) -> $ty {
            (self.$field $([$idx])?).get_bits(($offset)..(($offset) + ($length))) as $ty
        }
    };
}

#[macro_export]
macro_rules! setbit {
    ($vis:vis $name:ident ; $field:tt $([$idx:literal])? ; $offset:literal) => {
        $vis fn $name(&mut self, value: bool) {
            (self.$field $([$idx])?).set_bit($offset, value);
        }
    };
}

#[macro_export]
macro_rules! setbits {
    ($vis:vis $name:ident : $ty:ty ; $field:tt $([$idx:literal])? ; $offset:literal ; $length:literal) => {
        $vis fn $name(&mut self, value: $ty) {
            (self.$field $([$idx])?).set_bits(($offset)..(($offset) + ($length)), value.into());
        }
    };
}

#[macro_export]
macro_rules! withbit {
    ($vis:vis $name:ident ; $field:tt $([$idx:literal])? ; $offset:literal) => {
        $vis fn $name(mut self, value: bool) -> Self {
            (self.$field $([$idx])?).set_bit($offset, value);
            self
        }
    };
}

#[macro_export]
macro_rules! withbits {
    ($vis:vis $name:ident : $ty:ty ; $field:tt $([$idx:literal])? ; $offset:literal ; $length:literal) => {
        $vis fn $name(mut self, value: $ty) -> Self {
            (self.$field $([$idx])?).set_bits(($offset)..(($offset) + ($length)), value.into());
            self
        }
    };
}

#[cfg(test)]
mod tests {
    use bit_field::BitField;

    #[test]
    fn test_bit_field() {
        let x = 0x00112000;
        assert_eq!(x.get_bits(16..32) as u16, 17);
    }
}
