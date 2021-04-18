use core::mem::MaybeUninit;

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
pub enum ArrayVecError {
    NoSpace,
}

pub type Result<T> = core::result::Result<T, ArrayVecError>;

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
            Result::Err(ArrayVecError::NoSpace)
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

/// Fixed-sized array backed map.
pub struct ArrayMap<K, V, const N: usize> {
    buf: [Option<(K, V)>; N],
}

pub type ArrayMapResult<T> = core::result::Result<T, ArrayMapError>;

#[derive(Debug)]
pub enum ArrayMapError {
    NoSpace,
}

impl<K, V, const N: usize> ArrayMap<K, V, N>
where
    K: PartialEq,
{
    pub fn new() -> ArrayMap<K, V, N> {
        let mut array: [MaybeUninit<Option<(K, V)>>; N] = MaybeUninit::uninit_array();
        for elem in array.iter_mut() {
            *elem = MaybeUninit::new(None);
        }
        ArrayMap {
            buf: unsafe { MaybeUninit::array_assume_init(array) },
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> ArrayMapResult<Option<V>> {
        for elem in self.buf.iter_mut() {
            match elem {
                Some((k, _)) if k == &key => {
                    let prev = elem.take().map(|(_, v)| v);
                    *elem = Some((key, value));
                    return Ok(prev);
                }
                None => {
                    *elem = Some((key, value));
                    return Ok(None);
                }
                _ => {}
            }
        }
        Err(ArrayMapError::NoSpace)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        for elem in self.buf.iter() {
            if let Some((k, v)) = elem {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        for elem in self.buf.iter_mut() {
            if let Some((k, v)) = elem.take() {
                if &k == key {
                    return Some(v);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::util::{ArrayMap, ArrayVec};

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

    #[test]
    fn array_map_insert_get() {
        let mut m: ArrayMap<i32, &str, 3> = ArrayMap::new();
        assert_eq!(m.insert(1, "one").unwrap(), None);
        assert_eq!(m.insert(2, "two").unwrap(), None);
        assert_eq!(m.insert(3, "three").unwrap(), None);
        assert!(m.insert(4, "four").is_err());

        assert_eq!(m.get(&1), Some(&"one"));
        // overwrite
        assert_eq!(m.insert(1, "ooonnneee").unwrap(), Some("one"));
        assert_eq!(m.get(&1), Some(&"ooonnneee"));
    }

    #[test]
    fn array_map_remove() {
        let mut m: ArrayMap<i32, &str, 3> = ArrayMap::new();
        assert_eq!(m.insert(1, "one").unwrap(), None);
        assert_eq!(m.get(&1), Some(&"one"));

        assert_eq!(m.remove(&1), Some("one"));
        assert_eq!(m.get(&1), None);
    }
}
