#[derive(Copy, Clone, Debug)]
pub struct CircularTracker<T: Copy + Default, const N: usize> {
    pub index: usize,
    pub items: [T; N],
    pub len: usize,
}

impl<T: Copy + Default, const N: usize> CircularTracker<T, N> {
    pub fn new(items: &[T]) -> Self {
        let mut buf = [T::default(); N];
        buf[..items.len()].copy_from_slice(items);
        Self {
            items: buf,
            len: items.len(),
            index: 0,
        }
    }

    pub fn current(&self) -> T {
        self.items[self.index]
    }

    pub fn next(&mut self) -> T {
        self.index = (self.index + 1) % self.len;
        self.current()
    }

    pub fn prev(&mut self) -> T {
        self.index = (self.index + self.len - 1) % self.len;
        self.current()
    }
}
