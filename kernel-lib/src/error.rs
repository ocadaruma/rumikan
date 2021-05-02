#[derive(Debug)]
pub struct ErrorWithInfo<E> {
    error: E,
    file: &'static str,
    line: u32,
}

impl<E> ErrorWithInfo<E> {
    pub fn new(error: E, file: &'static str, line: u32) -> Self {
        Self { error, file, line }
    }
}
