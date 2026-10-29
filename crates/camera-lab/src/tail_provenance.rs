//! Bounded observations only; never lifecycle authority.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TailProvenance {
    pub initial_sync: u16,
    pub intervening_boundary: u16,
    pub intervening_overflow: u16,
    pub tail_bytes: u32,
    /// 0 empty, 1 incomplete length, 2 complete accepted, 3 complete unaccepted.
    pub tail_state: u8,
    pub saturated: bool,
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn v020_saturation_and_invalid_provenance_fail_closed() {
        let mut p=TailProvenance {intervening_boundary:u16::MAX,..Default::default()};
        p.assembly(true,false,1);assert!(p.saturated);assert_eq!(p.intervening_boundary,u16::MAX);
        p.finish(3145729,3145728,false);assert!(p.saturated);assert!(!p.valid());
        p.tail_bytes=0;p.tail_state=2;assert!(!p.valid());
        p.tail_state=7;assert!(!p.valid());
    }
}
impl TailProvenance {
    fn increment(n: &mut u16, saturated: &mut bool) {
        let (next, overflow) = n.overflowing_add(1);
        if overflow { *saturated = true; } else { *n = next; }
    }
    pub fn assembly(&mut self, was_synced: bool, overflow: bool, discarded: u64) {
        if overflow {
            Self::increment(&mut self.intervening_overflow, &mut self.saturated);
        }
        for _ in 0..discarded.saturating_sub(u64::from(overflow)).min(2) {
            if !was_synced && self.initial_sync == 0 {
                Self::increment(&mut self.initial_sync, &mut self.saturated);
            } else {
                Self::increment(&mut self.intervening_boundary, &mut self.saturated);
            }
        }
        if discarded > 2 { self.saturated = true; }
    }
    pub fn finish(&mut self, len: usize, size: usize, accepted: bool) {
        self.tail_bytes = len.min(0x3fffff) as u32;
        self.tail_state = if len == 0 { 0 } else if len == size {
            if accepted { 2 } else { 3 }
        } else { 1 };
        self.saturated |= len > 0x3fffff || len > size || (accepted && len != size);
    }
    pub fn valid(&self) -> bool {
        self.initial_sync <= 1 && self.tail_bytes <= 3145728 && match self.tail_state {
            0 => self.tail_bytes == 0,
            1..=3 => self.tail_bytes > 0,
            _ => false,
        }
    }
}
