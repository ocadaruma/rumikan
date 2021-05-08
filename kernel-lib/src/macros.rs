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
