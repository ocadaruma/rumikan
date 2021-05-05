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
