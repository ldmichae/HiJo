#[derive(Debug, Copy, Clone)]
pub struct CircularTracker<const N: usize, T>
where
    T: Default + Copy,
{
    pub index: usize,
    pub items: [T; N],
    pub len: usize,
}

impl<const N: usize, T: Default + Copy> CircularTracker<N, T> {
    pub fn new(items: &[T], init_index: Option<u8>) -> Self {
        let idx = init_index.unwrap_or_default();
        let mut buf = [T::default(); N];
        buf[..items.len()].copy_from_slice(items);
        Self {
            index: idx as usize,
            items: buf,
            len: items.len(),
        }
    }

    pub fn current(self) -> (usize, T) {
        let val = self.items[self.index];
        (self.index, val)
    }

    pub fn next(&mut self) -> (usize, T) {
        let next_idx = (self.index + 1) % self.len;
        self.index = next_idx;
        self.current()
    }

    pub fn previous(&mut self) -> (usize, T) {
        let next_idx = (self.index + self.len - 1) % self.len;
        self.index = next_idx;
        self.current()
    }
}
