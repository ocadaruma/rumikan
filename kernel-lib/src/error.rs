use core::fmt::{Debug, Formatter};

pub struct ErrorContext<E> {
    error: E,
    module_path: &'static str,
    line: u32,
}

impl<E> ErrorContext<E> {
    pub fn new(error: E, module_path: &'static str, line: u32) -> Self {
        Self { error, module_path, line }
    }
}

impl<E> Debug for ErrorContext<E> where E : Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?} at {}/L{}", self.error, self.module_path, self.line))?;
        Ok(())
    }
}
