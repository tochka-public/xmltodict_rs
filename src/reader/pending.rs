#[derive(Default)]
pub struct PendingBytes {
    buf: Vec<u8>,
    offset: usize,
}

impl PendingBytes {
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.offset = 0;
    }

    pub fn fill_from_slice(&mut self, bytes: &[u8]) {
        self.buf.clear();
        self.buf.extend_from_slice(bytes);
        self.offset = 0;
    }

    pub fn copy_into(&mut self, out: &mut [u8]) -> usize {
        let Some(remaining) = self.buf.get(self.offset..) else {
            self.clear();
            return 0;
        };

        let to_copy = remaining.len().min(out.len());
        let Some(dst) = out.get_mut(..to_copy) else {
            return 0;
        };
        let Some(src) = remaining.get(..to_copy) else {
            return 0;
        };
        dst.copy_from_slice(src);
        self.offset = self.offset.saturating_add(to_copy);
        if self.offset >= self.buf.len() {
            self.clear();
        }
        to_copy
    }
}
