/// Fixed-sized array-backed vector.
#[derive(Debug)]
pub struct ArrayVec<T, const N: usize>
where
    T: Copy + Default,
{
    buf: [T; N],
    len: usize,
}

#[derive(Debug)]
pub enum Error {
    CapacityFull,
}

pub type Result<T> = core::result::Result<T, Error>;

#[allow(clippy::len_without_is_empty)]
impl<T, const N: usize> ArrayVec<T, N>
where
    T: Copy + Default,
{
    pub fn new() -> ArrayVec<T, N> {
        ArrayVec {
            buf: [T::default(); N],
            len: 0,
        }
    }

    pub fn add(&mut self, value: T) -> Result<()> {
        if self.len < N {
            self.buf[self.len] = value;
            self.len += 1;
            Result::Ok(())
        } else {
            Result::Err(Error::CapacityFull)
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[T] {
        &self.buf[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.buf[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use crate::util::ArrayVec;

    #[test]
    fn array_vec_add() {
        let mut v = ArrayVec::<u32, 16>::new();
        v.add(42).unwrap();
        assert_eq!(v.as_slice(), &[42]);
    }

    #[test]
    fn array_vec_full() {
        let mut v = ArrayVec::<u32, 16>::new();
        for _ in 0..15 {
            v.add(42).unwrap();
        }
        assert!(v.add(43).is_ok());
        assert!(v.add(44).is_err());
    }
}
