use crate::error::ErrorContext;
use core::mem::size_of;

const MEMORY_POOL_BYTES: usize = 4096 * 32;
const MEMORY_POOL_ALIGNMENT_COUNT: usize = MEMORY_POOL_BYTES / 64;

static mut MEMORY_POOL: [Alignment; MEMORY_POOL_ALIGNMENT_COUNT] =
    [Alignment([0; 64]); MEMORY_POOL_ALIGNMENT_COUNT];
static mut OFFSET: usize = 0;

#[derive(Copy, Clone)]
#[repr(C, align(64))]
struct Alignment([u8; 64]);

#[derive(Debug)]
pub enum ErrorType {
    OutOfMemory,
}

pub type Error = ErrorContext<ErrorType>;
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
pub fn current_offset() -> usize {
    unsafe { OFFSET }
}

#[cfg(test)]
pub fn free_all() {
    unsafe {
        OFFSET = 0;
    }
}

pub fn allocate<T>(
    bytes: usize,
    alignment: Option<usize>,
    boundary: Option<usize>,
) -> Result<*mut T> {
    let mut offset = unsafe { OFFSET };
    if let Some(alignment) = alignment {
        offset = ceil(offset, alignment);
    }
    if let Some(boundary) = boundary {
        let next_boundary = ceil(offset, boundary);
        if offset + bytes > next_boundary {
            offset = next_boundary;
        }
    }

    if offset + bytes <= MEMORY_POOL_BYTES {
        unsafe {
            OFFSET = offset + bytes;
            let base_ptr: *mut [Alignment; MEMORY_POOL_ALIGNMENT_COUNT] = &mut MEMORY_POOL;
            let ptr = (base_ptr as *mut u8).add(offset);
            Ok(ptr as *mut T)
        }
    } else {
        Err(make_error!(ErrorType::OutOfMemory))
    }
}

pub fn allocate_array<T: Default>(
    len: usize,
    alignment: Option<usize>,
    boundary: Option<usize>,
) -> Result<*mut T> {
    allocate::<T>(len * size_of::<T>(), alignment, boundary).map(|ptr| unsafe {
        for i in 0..len {
            ptr.add(i).write(T::default());
        }
        ptr
    })
}

fn ceil(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

#[cfg(test)]
mod tests {
    use crate::usb::mem::{allocate, allocate_array, ceil, current_offset, free_all};
    use core::slice::from_raw_parts_mut;

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    struct TestStruct {
        a: u32,
        b: u64,
    }

    impl Default for TestStruct {
        fn default() -> Self {
            TestStruct { a: 0, b: 0 }
        }
    }

    #[test]
    fn allocate_without_alignment() {
        free_all();

        for _ in 0..32 {
            assert!(allocate::<()>(4096, None, None).is_ok());
        }
        assert!(allocate::<()>(4096, None, None).is_err());
    }

    #[test]
    fn test_contiguous_layout() {
        free_all();

        let first_ptr = allocate::<()>(1, None, None).unwrap() as u64;
        for i in 1..(4096 * 32) {
            let ptr = allocate::<()>(1, None, None).unwrap() as u64;
            assert_eq!(ptr - first_ptr, i);
        }
        assert!(allocate::<()>(1, None, None).is_err());
    }

    #[test]
    fn test_base_alignment() {
        free_all();

        let ptr = allocate::<()>(1, None, None).unwrap() as u64;
        assert_eq!(ptr % 64, 0);
    }

    #[test]
    fn allocate_alignment() {
        free_all();

        assert!(allocate::<()>(1, Some(64), Some(4096)).is_ok());
        assert_eq!(current_offset(), 1);

        assert!(allocate::<()>(1, Some(64), Some(4096)).is_ok());
        assert_eq!(current_offset(), 64 + 1);

        assert!(allocate::<()>(4090, Some(64), Some(4096)).is_ok());
        assert_eq!(current_offset(), 4096 + 4090);
    }

    #[test]
    fn allocate_array_test_struct() {
        free_all();

        let array: *mut TestStruct = allocate_array(2, None, None).unwrap();
        let array = unsafe { from_raw_parts_mut(array, 2) };

        assert_eq!(array.len(), 2);
        assert_eq!(array[0], TestStruct { a: 0, b: 0 });
        assert_eq!(array[1], TestStruct { a: 0, b: 0 });
    }

    #[test]
    fn ceil_64() {
        assert_eq!(ceil(0, 64), 0);
        assert_eq!(ceil(1, 64), 64);
        assert_eq!(ceil(63, 64), 64);
        assert_eq!(ceil(64, 64), 64);
        assert_eq!(ceil(65, 64), 128);
    }
}
