#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! make_error {
    ($err:expr) => {
        $crate::error::ErrorWithInfo::new($err, file!(), line!())
    };
}
