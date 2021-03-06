use core::mem::MaybeUninit;
use core::ops::{Index, IndexMut};

#[derive(Debug)]
pub enum CollectionError {
    NoSpace,
}

pub type Result<T> = core::result::Result<T, CollectionError>;

/// Fixed-sized array-backed vector.
#[derive(Debug)]
pub struct ArrayVec<T, const N: usize> {
    buf: [T; N],
    len: usize,
}

#[allow(clippy::len_without_is_empty)]
impl<T, const N: usize> ArrayVec<T, N> {
    pub fn new() -> ArrayVec<T, N> {
        let mut array: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
        for elem in array.iter_mut() {
            *elem = MaybeUninit::zeroed();
        }
        ArrayVec {
            buf: unsafe { MaybeUninit::array_assume_init(array) },
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<()> {
        if self.len < N {
            self.buf[self.len] = value;
            self.len += 1;
            Result::Ok(())
        } else {
            Result::Err(CollectionError::NoSpace)
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

impl<T, const N: usize> Index<usize> for ArrayVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.buf[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for ArrayVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.buf[index]
    }
}

/// Fixed-sized array backed map.
#[derive(Debug)]
pub struct ArrayMap<K, V, const N: usize> {
    buf: [Option<(K, V)>; N],
}

#[allow(clippy::new_without_default)]
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

    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>> {
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
        Err(CollectionError::NoSpace)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        for (k, v) in self.buf.iter().flatten() {
            if k == key {
                return Some(v);
            }
        }
        None
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        for (k, v) in self.buf.iter_mut().flatten() {
            if k == key {
                return Some(v);
            }
        }
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        for elem in self.buf.iter_mut() {
            match elem {
                Some((k, _)) if k == key => return elem.take().map(|(_, v)| v),
                _ => {}
            }
        }
        None
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V, N> {
        IterMut {
            inner: self.buf.as_mut(),
        }
    }
}

pub struct IterMut<'a, K, V, const N: usize> {
    inner: &'a mut [Option<(K, V)>],
}

impl<'a, K, V, const N: usize> Iterator for IterMut<'a, K, V, N> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        let mut entries = core::mem::replace(&mut self.inner, &mut []);
        while !entries.is_empty() {
            let (head, tail) = entries.split_first_mut().unwrap();
            if let Some((k, v)) = head {
                self.inner = tail;
                return Some((k, v));
            }
            entries = tail;
        }
        None
    }
}

#[derive(Debug)]
pub struct ArrayQueue<T, const N: usize> {
    enqueue_ptr: usize,
    len: usize,
    buf: [T; N],
}

#[allow(clippy::new_without_default)]
impl<T, const N: usize> ArrayQueue<T, N>
where
    T: Default,
{
    pub fn new() -> Self {
        let mut buf: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
        for elem in buf.iter_mut() {
            *elem = MaybeUninit::zeroed();
        }
        Self {
            enqueue_ptr: 0,
            len: 0,
            buf: unsafe { MaybeUninit::array_assume_init(buf) },
        }
    }

    pub fn push(&mut self, elem: T) -> Result<()> {
        if self.len < N {
            self.buf[self.enqueue_ptr] = elem;
            self.enqueue_ptr = (self.enqueue_ptr + 1) % N;
            self.len += 1;
            Ok(())
        } else {
            Err(CollectionError::NoSpace)
        }
    }

    pub fn poll(&mut self) -> Option<T> {
        if self.len > 0 {
            let dequeue_ptr =
                (((self.enqueue_ptr as isize - self.len as isize) + N as isize) as usize) % N;
            self.len -= 1;
            Some(core::mem::take(&mut self.buf[dequeue_ptr]))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::collection::{ArrayMap, ArrayQueue, ArrayVec};

    #[test]
    fn array_vec_add() {
        let mut v = ArrayVec::<u32, 16>::new();
        v.push(42).unwrap();
        assert_eq!(v.as_slice(), &[42]);
    }

    #[test]
    fn array_vec_full() {
        let mut v = ArrayVec::<u32, 16>::new();
        for _ in 0..15 {
            v.push(42).unwrap();
        }
        assert!(v.push(43).is_ok());
        assert!(v.push(44).is_err());
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
        assert_eq!(m.insert(2, "two").unwrap(), None);
        assert_eq!(m.insert(3, "three").unwrap(), None);
        assert_eq!(m.get(&1), Some(&"one"));
        assert_eq!(m.get(&2), Some(&"two"));
        assert_eq!(m.get(&3), Some(&"three"));

        assert_eq!(m.remove(&2), Some("two"));
        assert_eq!(m.get(&2), None);
        assert_eq!(m.get(&1), Some(&"one"));
        assert_eq!(m.get(&3), Some(&"three"));
    }

    #[test]
    fn array_map_iter_mut() {
        let mut m: ArrayMap<i32, &str, 3> = ArrayMap::new();
        m.insert(1, "one").unwrap();
        m.insert(2, "two").unwrap();
        m.insert(3, "three").unwrap();
        m.remove(&2).unwrap();

        assert_eq!(m.get(&1), Some(&"one"));
        let mut iter = m.iter_mut();
        assert_eq!(iter.next(), Some((&1, &mut "one")));
        assert_eq!(iter.next(), Some((&3, &mut "three")));
        assert!(iter.next().is_none());
    }

    #[test]
    fn array_queue() {
        let mut queue: ArrayQueue<i32, 3> = ArrayQueue::new();
        assert_eq!(queue.poll(), None);

        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_ok());
        assert!(queue.push(4).is_err());

        assert_eq!(queue.poll(), Some(1));
        assert_eq!(queue.poll(), Some(2));

        assert!(queue.push(4).is_ok());
        assert!(queue.push(5).is_ok());
        assert!(queue.push(6).is_err());

        assert_eq!(queue.poll(), Some(3));
        assert_eq!(queue.poll(), Some(4));
        assert_eq!(queue.poll(), Some(5));
        assert_eq!(queue.poll(), None);
    }
}
