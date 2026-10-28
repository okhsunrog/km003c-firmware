//! Two-slot ownership protocol: DMA never targets READY or READING memory.
//! The board adapter serializes these transitions between IRQ and consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Free,
    Filling,
    Ready,
    Reading,
}
pub struct Pool {
    slots: [Slot; 2],
    active: usize,
}
impl Default for Pool {
    fn default() -> Self {
        Self {
            slots: [Slot::Filling, Slot::Free],
            active: 0,
        }
    }
}
impl Pool {
    pub fn active(&self) -> usize {
        self.active
    }
    /// Publish the finished slot only if the other can immediately receive.
    /// Otherwise discard the newest burst and reuse its slot, preserving the
    /// older unread/borrowed burst. Caller counts the loss explicitly.
    pub fn complete(&mut self) -> Option<usize> {
        let old = self.active;
        let next = 1 - old;
        if self.slots[next] != Slot::Free {
            return None;
        }
        self.slots[old] = Slot::Ready;
        self.slots[next] = Slot::Filling;
        self.active = next;
        Some(old)
    }
    pub fn other_is_reading(&self) -> bool {
        self.slots[1 - self.active] == Slot::Reading
    }
    pub fn ready(&self) -> bool {
        self.slots.contains(&Slot::Ready)
    }
    pub fn acquire(&mut self) -> Option<usize> {
        let i = self.slots.iter().position(|s| *s == Slot::Ready)?;
        self.slots[i] = Slot::Reading;
        Some(i)
    }
    pub fn release(&mut self, i: usize) {
        assert_eq!(self.slots[i], Slot::Reading);
        self.slots[i] = Slot::Free;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_reader_is_never_overwritten() {
        let mut p = Pool::default();
        assert_eq!(p.complete(), Some(0));
        assert_eq!(p.active(), 1);
        for _ in 0..100 {
            assert_eq!(p.complete(), None);
            assert_eq!(p.active(), 1);
        }
        assert_eq!(p.acquire(), Some(0));
        assert!(!p.ready());
        for _ in 0..100 {
            assert_eq!(p.complete(), None);
        }
        p.release(0);
        assert_eq!(p.complete(), Some(1));
        assert_eq!(p.active(), 0);
        assert_eq!(p.acquire(), Some(1));
        p.release(1);
    }
    #[test]
    fn fast_reader_alternates_indefinitely() {
        let mut p = Pool::default();
        for n in 0..1000 {
            let i = n % 2;
            assert_eq!(p.complete(), Some(i));
            assert_eq!(p.acquire(), Some(i));
            p.release(i);
        }
    }
}
