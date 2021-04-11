// TRB is a 16-bytes fixed length bit string.
// We use u128 to represent single TRB

use crate::usb::mem::allocate_array;

#[derive(Debug)]
pub struct Ring {
    buffer: *mut u128,
    len: usize,
    cycle_bit: bool,
    write_index: usize,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    AllocError(crate::usb::mem::Error),
}

impl Ring {
    pub fn new() -> Ring {
        Ring {
            buffer: core::ptr::null_mut(),
            len: 0,
            cycle_bit: false,
            write_index: 0,
        }
    }

    pub fn initialize(&mut self, len: usize) -> Result<()> {
        self.cycle_bit = true;
        self.write_index = 0;
        self.len = len;

        match allocate_array::<u128>(len, Some(64), Some(64 * 1024)) {
            Ok(ptr) => {
                self.buffer = ptr;
                Ok(())
            }
            Err(err) => Err(Error::AllocError(err)),
        }
    }

    pub fn ptr_as_u64(&self) -> u64 {
        self.buffer as u64
    }
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}
