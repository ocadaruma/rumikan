#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! mkerror {
    ($err:expr) => {
        $crate::error::ErrorContext::new($err, module_path!(), line!())
    };
}

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
